# Description
```
This service tests performance of BTC and EVM nodes by replicating real-world patterns with specific JSON-RPC payloads. 
```

# Usage
After making the .env file based on the .env.example provided, use
```
docker-compose up
```
in this project's directory
# Service Architecture
![Benchmarking service architecture](./benchmarking_service_scheme.png)


## REGISTER JOB

### POST /jobs

#### Request Example 1:
```
{
	"chain": "EVM",
    "endpoint": "https://endpoints.omniatech.io/v1/<chain>/<endpoint-uuid>"
	"threads": 20,
	"duration": 60
}
```
#### Request Example 2:
```
{
	"chain": "BTC",
    "endpoint": "https://endpoints.omniatech.io/v1/<chain>/<endpoint-uuid>"
	"threads": 10,
	"duration": 60
}
```
#### Request Example 3:
```
{
	"chain": "EVM",
    "endpoint": "https://endpoints.omniatech.io/v1/<chain>/<endpoint-uuid>"
	"threads": 20,
	"duration": 60,
	"authorization": "Basic YWxleDoxMjM0"
}
```
#### Request Example 4:
```
{
	"chain": "EVM",
    "endpoint": "https://endpoints.omniatech.io/v1/<chain>/<endpoint-uuid>"
	"threads": 10,
	"duration": 60,
	"authorization": "Bearer MHnQx2fd4714ooTXZTq9"
}
```
#### Response Example 1:
```
201 OK
{
	"id": "l4xt7lgaMdJvBF9K8cO6w4u7djc0pH"
}
```
#### Response Example 2:
```
400 BadRequest
{
    "error": "Failed to deserialize TodoJob object Cause: missing field <field_name>"
}
```
#### Response Example 3:
```
500 InternalServerError
{
    "error": "<message>"
}
```
## GET JOB BY ID
### GET /jobs/{job_id}
#### Request Example:
```
GET /jobs/l4xt7lgaMdJvBF9K8cO6w4u7djc0pH
```
#### Response Example 1:
```
200 OK
{
	"status": "PENDING"
}
```
#### Response Example 2:
```
200 OK
{
	"status": "ERRORED"
}
```
#### Response Example 3:
```
200 OK
{
	"status": "FINISHED",
	"rps": 70
}
```
#### Response Example 4:
```
404 NotFound
<empty-body>
```
#### Response Example 5:
```
500 InternalServerError
{
    "error": "<message>"
}
```