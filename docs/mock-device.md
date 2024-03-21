# Mock Device Extension

The mock device extension provides functionality that allows Ripple to be run without an underlying device. This is useful for testing and certain development workloads where the a real device is not actually needed and the interactions with the device are known.

The operation of the extension is quite simple. Once the extension loads it looks up the PlatformParameters from the device manifest. These parameters contain the platform gateway url. The extension takes this URL and starts up a websocket server at the gateway which leads to extensions that fulfill the device contracts connecting to this websocket server instead of the service that would be running on a real device. This websocket server contains a registry of requests and responses that determine the behaviour of interactions. When the server receives a request, it is looked up in the registry. If a match is found the corresponding request is sent back to the client. The extension also offers an interface through the Ripple WS Gateway to control the data contained within the registry.

## Extension Manifest

There is an example manifest in the `examples` folder that shows how to get the mock_device extension setup. The file is called `mock-thunder-device.json`. The important part of this file is the libmock_device entry in the `extns` array.

```json
{
            "path": "libmock_device",
            "symbols": [
                {
                    "id": "ripple:channel:device:mock_device",
                    "config": {
                        "mock_data_file": "mock-device.json",
                        "activate_all_plugins": "true"
                    },
                    "uses": [
                        "config"
                    ],
                    "fulfills": [
                    ]
                },
                {
                    "id": "ripple:extn:jsonrpsee:mock_device",
                    "uses": [
                        "ripple:channel:device:mock_device"
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

The file contains a request and response map with additional support for events with delay.
```json
{
    "org.rdk.System.1.getSystemVersions": [
            {
                "result": {
                    "receiverVersion": "6.9.0.0",
                    "stbTimestamp": "Tue 07 Nov 2023 00:03:20 AP UTC",
                    "stbVersion": "SCXI11BEI_VBN_23Q4_sprint_20231107000320sdy_FG_EDGE_R2PB_NG",
                    "success": true
                }
            }
        ],
    "org.rdk.System.register": [
    {
      "params": {
        "event": "onTimeZoneDSTChanged",
        "id": "client.org.rdk.System.events"
      },
      "result": 0,
      "events": [
        {
          "delay": 0,
          "data": {
            "oldTimeZone": "America/New_York",
            "newTimeZone": "Europe/London",
            "oldAccuracy": "INITIAL",
            "newAccuracy": "FINAL"
          }
        }
      ]
    },
    {
      "params": {
        "event": "onSystemPowerStateChanged",
        "id": "client.org.rdk.System.events"
      },
      "result": 0,
      "events": [
        {
          "delay": 0,
          "data": {
            "powerState": "ON",
            "currentPowerState": "ON"
          }
        }
      ]
    }
  ]

}
```

By default, this file is looked for in the ripple persistent folder under the name `mock-device.json` e.g. `~/.ripple/mock-device.json`. The location of this file can be controlled with the config setting in the channel sysmobl of the extensions manifest entry e.g. 

```json
{
    "id": "ripple:channel:device:mock_device",
    "config": {
        "mock_data_file": "<path>/mock-device.json"
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
    "id": 1,
    "method": "mockdevice.addRequests",
    "params": {
        "org.rdk.DisplaySettings.1.getCurrentResolution": [
    {
      "params": {
        "videoDisplay": "HDMI0"
      },
      "result": {
        "resolution": "2160p",
        "success": true
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
    "method": "device.screenResolution"
}
```

Response:
```json
{
    "jsonrpc": "2.0",
    "result": [
        3840,
        2160
    ],
    "id": 1
}
```

### RemoveRequest

Removes a request from the registry.

Payload:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "mockdevice.removeRequests",
    "params": {
        "org.rdk.DisplaySettings.1.getCurrentResolution": [
    {
      "params": {
        "videoDisplay": "HDMI0"
      },
      "result": {
        "resolution": "2160p",
        "success": true
      }
    }
  ]
    }
}
```

### Emitting Events
Mock device extension can also provide ability to emit events for an existing register Thunder listener.
Below is an example of emitting screen resolution event.

```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "mockdevice.emitEvent",
    "params": {
        "event": {
        "body": {
            "jsonrpc": "2.0",
            "method": "client.org.rdk.DisplaySettings.events.resolutionChanged",
            "params": {
                "width": 3840,
                "height": 2160,
                "videoDisplayType": "HDMI0",
                "resolution": "2160p"
            }
        },
        "delay": 0
        }
    }
}
```

## Payload types

Payload types MUST match the original schema definition from the mock data file.

# TODO

What's left?

- Integration tests for the mock device extension
- Unit tests covering extension client interactions