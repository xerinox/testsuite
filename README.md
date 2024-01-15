# testsuite
Rust cli test server with configurable responses

```
Usage: testsuite [OPTIONS]

Options:
  -p, --port <PORT>                  Port to run on [default: 8080]
  -f, --format <FORMAT>              Response format [default: json] [possible values: json, html]
  -C, --content-file <CONTENT_FILE>  Response content file
  -c, --content <CONTENT>            Response content
  -e, --endpoint <ENDPOINT>          [default: /]
  -h, --help                         Print help
  -V, --version                      Print version
```

# examples:
## Json file:

Create a json file: <path>/example.json
```json
{
    "is_test_object": true
}
```

executing `testsuite -C="<path>/example.json"` will start up a server on 127.0.0.1:8080, listening for incoming requests to the endpoint /example, and return a http response with the json data


## Text json response and setting endpoint
executing `testsuite -c="{ \"id\": 1 }" -e="/id"` will start up the server on 127.0.0.1:8080, listening for requests on /id, and returning http response with the json data.
