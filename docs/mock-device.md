# Mock Device Extension

The mock device extension provides functionality that allows Ripple to be run without an underlying device. This is useful for testing and certain development workloads where the a real device is not actually needed and the interactions with the device are known.

The operation of the extension is quite simple. Once the extension loads it looks up the PlatformParameters from the device manifest. These parameters contain the platform gateway url. The extension takes this URL and starts up a websocket server at the gateway which leads to extensions that fulfill the device contracts connecting to this websocket server instead of the service that would be running on a real device. This websocket server contains a registry of requests and responses that determine the behaviour of interactions. When the server receives a request, it is looked up in the registry. If a match is found the corresponding request is sent back to the client. The extension also offers an interface through the Ripple WS Gateway to control the data contained within the registry.

## Extension Manifest

There is an example manifest in the `examples` folder that shows how to get the mock_device extension setup. The file is called `extn-manifest-mock-device-example.json`. The important part of this file is the libmock_device entry in the `extns` array.

```json
{
    "path": "libmock_device",
    "symbols": [
        {
            "id": "ripple:channel:device:mock_device",
            "config": {
                "mock_data_file": "mock-device.json"
            },
            "uses": [
                "config"
            ],
            "fulfills": [
                "extn_provider.mock_device"
            ]
        },
        {
            "id": "ripple:extn:jsonrpsee:mock_device",
            "uses": [
                "extn_provider.mock_device"
            ],
            "fulfills": [
                "json_rpsee"
            ]
        }
    ]
}
```

The extension has two symbols in it. One for the websocket server channel and the other to add the RPC methods for controlling the mock server to the Ripple gateway.

Once your extn manifest has been updated to include this entry you will be able to run ripple on a machine that does not have the platform service running.

## Usage

### Initial mocks

Due to timing requirements of platform integrations it is often the case that you will need mock data in the server as soon as it starts, rather than adding it at runtime. This is prevents platform integration extensions from crashing when they make requests to the platform durin initilization. The mock device extension supports this use case by allowing the user to stored their mock data in a JSON file which will be loaded and passed to the websocket server before it starts accepting requests.

The file should contain a single array in JSON that represents the set of requests and responses that are expected from the device. For example:
```json
[
    {
        // the request to match
        "request": {
            // the type that the request should be interpretted as
            "type": "jsonrpc",
            // the body of the request to match
            "body": {
                "jsonrpc": "2.0",
                "id": 0,
                "method": "Controller.1.register",
                "params": {
                    "event": "statechange",
                    "id": "client.Controller.1.events"
                }
            }
        },
        // the list of responses which should be sent back to the client
        "responses": [
            {
                "type": "jsonrpc",
                "body": {
                    "jsonrpc": "2.0",
                    "id": 0,
                    "result": 0
                }
            }
        ]
    },
]
```

By default, this file is looked for in the ripple persistent folder under the name `mock-device.json` e.g. `~/.ripple/mock-device.json`. The location of this file can be controlled with the config setting in the channel sysmobl of the extensions manifest entry e.g. 

```json
{
    "id": "ripple:channel:device:mock_device",
    "config": {
        "mock_data_file": "mock-device.json"
    },
    ...
}
```

If the path is absolute it will be loaded from there. Otherwise the config entry will be appended to the `saved_dir` config setting from the device manifest.

An example for the Thunder platform can be found at `examples/mock-data/thunder-device.json`.


### Runtime mocks

Once Ripple is running the the mock device extension is loaded you will be able to add new mock data into the server using the following APIs. You must establish a websocket connection to ripple on the port being used for app connections (by default `3474`). You can use a dummy appId for this connection. An example gateway URL would be: `ws://127.0.0.1:3474?appId=test&session=test`. Once connected you can make JSON-RPC calls to the mock_device extension.

### AddRequestResponse

Used to add a request and corresponding responses to the mock server registry. If the request being added is already present, the responses will be overitten with the new data.

Payload:
```json
{
    "jsonrpc": "2.0",
    "id": 24,
    "method": "mockdevice.addRequestResponse",
    "params": {
        // incoming request to match
        "request": {
            "type": "jsonrpc",
            "body": {
                "jsonrpc": "2.0",
                "id": 0,
                "method": "org.rdk.System.1.getSystemVersions"
            }
        },
        "responses": [
            // expected response from the platform
            {
                "type": "jsonrpc",
                "body": {
                    "jsonrpc": "2.0",
                    "id": 0,
                    "result": {
                        "stbVersion": "AX061AEI_VBN_1911_sprint_20200109040424sdy",
                        "receiverVersion": "3.14.0.0",
                        "stbTimestamp": "Thu 09 Jan 2020 04:04:24 AM UTC",
                        "success": true
                    }
                }
            }
        ]
    }
}
```

If you submit the example payload above, you will then be able to submit a device.version request on the same connection and you will get the Firebolt response populated by data from the mock_device.

Request:
```json
{
    "jsonrpc": "2.0",
    "id": 5,
    "method": "device.version"
}
```

Response:
```json
{
    "jsonrpc": "2.0",
    "result": {
        "api": {
            "major": 0,
            "minor": 14,
            "patch": 0,
            "readable": "Firebolt API v0.14.0"
        },
        "firmware": {
            "major": 3,
            "minor": 14,
            "patch": 0,
            "readable": "AX061AEI_VBN_1911_sprint_20200109040424sdy"
        },
        "os": {
            "major": 0,
            "minor": 8,
            "patch": 0,
            "readable": "Firebolt OS v0.8.0"
        },
        "debug": "0.8.0 (02542a1)"
    },
    "id": 5
}
```

### RemoveRequest

Removes a request from the registry.

Payload:
```json
{
    "jsonrpc": "2.0",
    "id": 24,
    "method": "mockdevice.removeRequest",
    "params": {
        "request": {
            "type": "jsonrpc",
            "body": {
                "jsonrpc": "2.0",
                "id": 0,
                "method": "org.rdk.System.1.getSystemVersions"
            }
        }
    }
}
```

## Payload types

Currently two payload types for the mock data are supported:

- JSON (json): requests using this type will match verbatim to incoming requests. If an incoming request is not matched no response will be sent.
- JSON RPC (jsonrpc): request using this type will ignore the "id" field for matching and any responses sent will be amended with the incoming request id. If an incoming request is not matched a JSON RPC error response will be sent back.


# TODO

What's left?

- Support for emitting device events
- Integration tests for the mock device extension
- Unit tests covering extension client interactions