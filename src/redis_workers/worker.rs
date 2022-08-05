
use redis::{AsyncCommands, RedisError, Client};
use rsmq_async::{Rsmq, RsmqConnection, RsmqMessage};
use serde_json;
use slog::{info, Logger, warn, error};
use tokio::{task::JoinError, time::Instant};
use reqwest;
use crate::models;
use futures::{self};
use rand::Rng;


pub async fn start_worker(
    redis_options: rsmq_async::RsmqOptions,
    log: Logger,
    fail_percentage_treshold: f64
) -> () {
    // Connect to Redis db needed to sync workers and to schedule jobs
    let redis_client = Client::open(format!("redis://{}", redis_options.host.clone())).unwrap();
    let mut rsmq = Rsmq::new_with_connection(redis_options.clone(), redis_client.get_async_connection().await.unwrap());
    let mut redis_connection_manager = redis::aio::ConnectionManager::new(redis_client.clone()).await.unwrap();
    
    // Generate various payloads for the json-rpc requests that will be sent concurrently
    let eth_rpc_methods = gen_eth_json_rpc_methods();
    let btc_rpc_methods = gen_btc_json_rpc_methods();
    loop {
        // Redis-worker receives the new TodoJob through RSMQ from the web server (actix thread)
        let rsmq_msg: Result<Option<RsmqMessage<String>>, _> = rsmq.receive_message("jobs_q", None).await;
        let rsmq_msg = rsmq_msg.unwrap();
        if rsmq_msg.is_none() {
            continue;
        }
        let check_rps: Result<i64, RedisError> = redis_connection_manager.get(rsmq_msg.clone().unwrap().id).await;
        if check_rps.is_err() {
            continue;
        }
        let check_rps = check_rps.unwrap();
        if check_rps >= 0 { // 0 => job already allocated, >0 => job finished
            continue;
        }
        let rsmq_msg = rsmq_msg.unwrap();
        let job_id = rsmq_msg.id.as_str();

        // Set this job as allocated such that it guarantees only this worker will execute it
        // Note: the atomicity of this step is guaranteed by the SET command of Redis
        let res: Result<String, RedisError> = redis_connection_manager.set(job_id, 0).await;

        // These will handle the concurrent tasks launched by the worker as requested in the TodoJob body
        let mut concurrent_threads_handlers: Vec<actix_web::rt::task::JoinHandle<(f64, f64)>> = Vec::new();
        let job: models::TodoJob = serde_json::from_str(rsmq_msg.message.as_str()).unwrap();
        
        // Apply prority-based randomness to the payloads send by the concurrent threads
        // in order to replicate a real-world scenario as precisely as possible
        let mut rpc_payloads: Vec<&str> = Vec::with_capacity(job.duration as usize * 2000);
        select_rpc_payloads(&mut rpc_payloads, &eth_rpc_methods, &btc_rpc_methods, &job.chain, log.clone());
        
        let client = reqwest::Client::new();
        for _i in 0..job.num_threads {
            let thread_log = log.clone();
            let job = job.clone();
            let client_thread = client.clone();
            let thread_rpc_payloads = rpc_payloads.clone(); //TODO: Optimization needed. Pass &rpc_payloads to spawned threads without cloning (Node: see scoped threads/crossbeam crate)
            let start = Instant::now();
            concurrent_threads_handlers.push(
                actix_web::rt::spawn( 
                    async move {
                        execute_job(&thread_log, &job, &start, &client_thread, &thread_rpc_payloads).await
                    }
                )
            );
        }

        // Worker waits for TodoJob's num_threads to finish
        let join_results = futures::future::join_all(concurrent_threads_handlers).await;
        
        // Check if the fails treshold is exceeded and mark job as failed (-2) or successfull (measured rps)
        let (exceeded_treshold, rps) = job_fails_exceed_treshold(join_results, fail_percentage_treshold, log.clone());
        if exceeded_treshold {
            let res: Result<String, RedisError> = redis_connection_manager.set(job_id, -2).await;
        } else {
            let res: Result<String, RedisError> = redis_connection_manager.set(job_id, rps).await;
        }
        
        // Only now we can delete the job from RSMQ
        let res = rsmq.delete_message("jobs_q", job_id).await.unwrap();
    }

}

async fn execute_job(
    log: &Logger, 
    job: &models::TodoJob, 
    start: &Instant, 
    client: &reqwest::Client,
    rpc_payloads: &Vec<&'static str>
) -> (f64, f64) {
    let mut ok_s: f64 = 0.0;
    let mut fails: f64 = 0.0;
    let mut rpc_payload_index = 0;
    loop {
        if start.elapsed().as_secs() >= job.duration as u64 {
            break;
        }
        // Basically turns rpc_payloads Vec into a circular list 
        if rpc_payload_index == rpc_payloads.len() - 1 {
            rpc_payload_index = 0;
        }
        let mut request = client.post(job.endpoint_url.clone())
                                        .body(*rpc_payloads.get(rpc_payload_index).unwrap())
                                        .header("Content-Type", "application/json");
        if job.authorization.is_some() {
            request = request.header("Authorization", job.clone().authorization.unwrap())
        }
        match request.send().await {
            Ok(response) => 
                                    {
                                        if response.status().is_success() { 
                                            ok_s += 1.0;
                                        } else {
                                            fails += 1.0; // counts "HTTP 429 - Too many requests in a given amount of time." errors
                                        }
                                    },
            Err(_) => {
                        fails += 1.0; // counts "No response -> connection closed" errors
                    }
        }
    }
    // return successful and failed requests for job
    (ok_s, fails)
}


fn job_fails_exceed_treshold(
    results: Vec<Result<(f64, f64), JoinError>>,
    fail_percentage_treshold: f64,
    log: Logger
) -> (bool, i64) 
{
    let mut ok_s: f64 = 0.0;
    let mut fails: f64 = 0.0;
    for res in results {
        if res.is_err() {
            continue;
        }
        let res = res.unwrap();
        ok_s += res.0;
        fails += res.1;
    }
    let fails_percentage = fails/(ok_s+fails) * 100.0;
    if fails_percentage >= fail_percentage_treshold {
        return (true, fails.floor() as i64);
    }

    (false, ok_s.floor() as i64)
}


fn select_rpc_payloads(
    rpc_payloads: &mut Vec<&str>,
    evm_rpc_methods: &Vec<models::JsonRpcMethod>,
    btc_rpc_methods: &Vec<models::JsonRpcMethod>,
    chain: &str,
    log: Logger
) -> () {
    if chain.eq("BTC") {
        for _i in 0..rpc_payloads.capacity() {
            let index = select_index_using_weighted_cdf(btc_rpc_methods, log.clone());
            if index < 0 {
                continue;
            }
            let index = index as usize;
            rpc_payloads.push(btc_rpc_methods.get(index).unwrap().payload);
        }
    }
    if chain.eq("EVM") {
        for _i in 0..rpc_payloads.capacity() {
            let index = select_index_using_weighted_cdf(evm_rpc_methods, log.clone());
            if index < 0 {
                continue;
            }
            let index = index as usize;
            rpc_payloads.push(evm_rpc_methods.get(index).unwrap().payload);
        }
    }
}

fn select_index_using_weighted_cdf(
    rpc_methods: &Vec<models::JsonRpcMethod>,
    log: Logger
) -> i32 {
    // Compute cumulative weights
    let mut weights_sum = 0;
    let mut cdf_weights: Vec<u32> = Vec::with_capacity(rpc_methods.len());
    for _i in 0..rpc_methods.len() {
        if _i > 0 {
            cdf_weights.push(cdf_weights[_i - 1] + rpc_methods[_i].weight);
        } else {
            cdf_weights.push(rpc_methods[_i].weight);
        }
        weights_sum += rpc_methods[_i].weight;
    }
    // Pick a random number in the range of cumutalive weights
    let index = rand::thread_rng().gen_range(0..weights_sum);

    // Search rpc_method index corresponding to the previous index
    for _i in 0..cdf_weights.len() {
        if index <= cdf_weights[_i] {
            return _i as i32;
        }
    }
    return -1;
}


fn gen_btc_json_rpc_methods() -> Vec<models::JsonRpcMethod> {
    let mut btc_methods = Vec::<models::JsonRpcMethod>::new();
    btc_methods.push(
            models::JsonRpcMethod {
                payload:
                    r#"{
                        "jsonrpc": "2.0",
                        "id": "1",
                        "method": "sendrawtransaction",
                        "params": ["01000000010b4d12cf890540c116463510fa823188a648ce7539b6a9ceb454bfbe8da447d7230000006b48304502210095d4cf3d7dcffaf50354ad3fd6e909e6c81156ac8f26b4a972c178e1c6b886b802206c6d3287d2a1bd9aa9f16187bf49ec24581d2b471e222d24babfd511d83bf29601210242581ee416579a142b436a2ef5ef0e117941fe7a2998d2d34c9f476233080f48ffffffff02a6580100000000001976a91476c37e0cc46f856092164f2fad78dbfc7de8c87e88ac3fc30f000000000017a91422603b24d6bc97d390793ec58de38222fcccae328700000000"]
                    }"#,
                weight: 16
            }   
    );
    btc_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": "1",
                    "method": "logging",
                    "params": [["all"], ["libevent"]]
                }"#,
            weight: 252
        }
    );
    btc_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": "1",
                    "method": "gettxout",
                    "params": ["47df2d439a7f7156da11a01478ea921c9fabc0f55a9f901291dccc762b40a937", 1]
                }"#,
            weight: 255
        }
    );
    btc_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": "1",
                    "method": "getblock", 
                    "params": ["00000000c937983704a73af28acdec37b049d214adbda81d7e2a3dd146f6ed09"]
                }"#,
            weight: 333
        }   
    );
    btc_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": "1",
                    "method": "getblockstats",
                    "params": [103221, []]
                }"#,
            weight: 390
        }
    );




    btc_methods
}


fn gen_eth_json_rpc_methods() -> Vec<models::JsonRpcMethod> {
    let mut eth_methods = Vec::<models::JsonRpcMethod>::new();
    eth_methods.push(
            models::JsonRpcMethod {
                payload: 
                    r#"{
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "eth_sendRawTransaction",
                        "params": ["0xf86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83"]
                    }"#,
                weight: 16
            }
    );
    eth_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "eth_getCode",
                    "params": ["0x5B56438000bAc5ed2c6E0c1EcFF4354aBfFaf889","latest"]
                }"#,
            weight: 88
        }
    );
    eth_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "eth_getLogs",
                    "params": [{"address": "0xdAC17F958D2ee523a2206206994597C13D831ec7"}]
                }"#,
            weight: 252
        }
    );
    eth_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "eth_getTransactionByHash",
                    "params": ["0x04b713fdbbf14d4712df5ccc7bb3dfb102ac28b99872506a363c0dcc0ce4343c"]
                }"#,
            weight: 255
        }
    );
    eth_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "eth_blockNumber",
                    "params": []
                }"#,
            weight: 333
        }
    );
    eth_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "eth_getTransactionCount",
                    "params": ["0x8D97689C9818892B700e27F316cc3E41e17fBeb9", "latest"]
                }"#,
            weight: 390
        }
    );
    eth_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "eth_getBlockByNumber",
                    "params": ["0xc5043f",false]
                }"#,
            weight: 399
        }
    );
    eth_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "eth_getBalance",
                    "params": ["0x8D97689C9818892B700e27F316cc3E41e17fBeb9", "latest"]
                }"#,
            weight: 545
        }
    );
    eth_methods.push(
        models::JsonRpcMethod {
            payload: 
                r#"{
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "eth_getTransactionReceipt",
                    "params": ["0x04b713fdbbf14d4712df5ccc7bb3dfb102ac28b99872506a363c0dcc0ce4343c"]
                }"#,
            weight: 607
        }
    );
    eth_methods.push(
        models::JsonRpcMethod {
            payload:
                r#"{
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "eth_call",
                    "params": [{"from":null,"to":"0x6b175474e89094c44da98b954eedeac495271d0f","data":"0x70a082310000000000000000000000006E0d01A76C3Cf4288372a29124A26D4353EE51BE"}, "latest"]
                }"#,
            weight: 1928
        }
    );
    return eth_methods;
}
