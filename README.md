# Service Architecture
![Benchmarking service architecture](./benchmarking_service_scheme.png)


## REGISTER JOB

### POST /jobs

#### Request Example 1:
```
{
	"chain": "EVM",
    "endpoint": "http://8.8.8.8"
	"threads": 20,
	"duration": 60
}
```
#### Request Example 2:
```
{
	"chain": "BTC",
    "endpoint": "https://my.private.node:8545"
	"threads": 10,
	"duration": 60
}
```
#### Request Example 3:
```
{
	"chain": "EVM",
    "endpoint": "http://127.0.0.1"
	"threads": 20,
	"duration": 60,
	"authorization": "Basic YWxleDoxMjM0"
}
```
#### Request Example 4:
```
{
	"chain": "EVM",
    "endpoint": "http://12.42.5.1:9000"
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