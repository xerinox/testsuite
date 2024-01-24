# testsuite
Simple cli test server with configurable responses and endpoints

```
Usage: testsuite [OPTIONS]

Options:
  -p, --port <PORT>
          Port to run on [default: 8080]
  -c, --content <CONTENT>
          Response content
      --content-file <CONTENT_FILE>
          Response content file
      --content-folder <CONTENT_FOLDER>
          Response content folder (All json/html files will be endpoints with file name as path)

  -f, --format <FORMAT>
          Response format [default: json] [possible values: json, html]
  -e, --endpoint <ENDPOINT>
          [default: /]
  -a, --allow-remote
        Allows remote connections to the server
  -h, --help
          Print help
  -V, --version
          Print version
```

# TUI 
Contains two views, address list (incoming connection IPs) and details list, showing each connection in more detail.
Trying to press enter on a detail will panic with todo.


- <kbd>Up</kbd> / <kbd>Down</kbd> Move in list
- <kbd>Enter</kbd> - Select item for further inspection
- <kbd>Esc</kbd> - Go back to previous view



## Examples:
### Json file:

Create a json file: <path>/example.json
```json
{
    "is_test_object": true
}
```

executing `testsuite --content-file="<path>/example.json"` will start up a server on 127.0.0.1:8080, listening for incoming requests to the endpoint /example, and return a http response with the json data

### Text json response and setting endpoint
executing `testsuite -c="{ \"id\": 1 }" -e="/id"` will start up the server on 127.0.0.1:8080, listening for requests on /id, and returning http response with the json data.

### Several endpoints: 
`testsuite --content-folder="<path>/"` will start up a server on 127.0.0.1:8080, and each html/json file in the folder will be an endpoint with their file name(without extension) as the endpoint address and http Content-Type matching the extension

