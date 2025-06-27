# Passthrough RPC in Ripple 

 

## Problem 

Some firebolt APIs can be completely encapsulated in an extension/plugin. The open source Ripple provides no middleware logic, composition, or data mapping. For these APIs, we should avoid as much (boilerplate) code in Ripple as possible. 

This can be done as configuration instead of code. 

## Solution 

Support config for Ripple in the device manifest file

```
{ 
  "passthrough_rpcs": [ 
    { 
      "method": "device.model", 
      "url": "ws://127.0.0.1:9992/jsonrpc", 
      "protocol": "jsonrpc", 
      "include_context": ["appId"] 
    } 
  ] 
} 
```

At the gateway level (before any Ripple handler code), Ripple will check if the called method is a passthrough rpc. If it is then it just sends the exact jsonrpc using the given protocol through the given url. Ripple will first only support jsonrpc as a persistent websocket connection with pass-through jsonrpc. 
Possible support in future for comrpc, and would need to come up with a common algorithm for mapping jsonrpc to comrpc.

### Context passing

Only Ripple knows the context of calling application. We may want the plugin that implements the jsonrpc call to need to know who the calling app is. 

The array "include_context" tells Ripple it needs to include some context when passing it to the implementing component. 

If an app makes the call: 
```
{ 
  "jsonrpc": "2.0", 
  "id": 1, 
  "method": "device.model" 
} 
```

Then if include_context contains "appId", Ripple will send this message through the jsonrpc websocket: 

```
{ 
  "jsonrpc": "2.0", 
  "id": 1, 
  "method": "device.model",
  "params": {
    "__ctx": {
      "appId": "test.firecert"
    }
  }
} 
```

`__ctx` is a reserved parameter name that all passthrough providers must handle. `__ctx` will be left undefined if `include_context` is undefined or is an empty list.

Only `appId` will be supported for first implementation, but the configuration specification can support other ctx keys in the future.

 ### Pass-through RPC as override

 Ripple must be certified for all firebolt APIs that are in the firebolt specification. So Ripple should not be completely dependent on configured pass-throughs. Ripple thus can still be the provider of the rpc handler for the APIs. `passthrough_rpcs` would be an override of those existing handlers.

 If there is no registered passthrough rpc and no Ripple handler defined, then Ripple should return an error back to the app with a not supported error.  

Ripple should fail to start if a MUST capability in Firebolt does not have a corresponding passthrough or handler.

### Permission and grant checking

The passthrough provider does not need to worry about permission checking, Ripple will continue to do that. Any request from Ripple should be considered already checked as far as permissions.

## Open questions

### Correlation of Requests and Response from the passthrough provider

Since the jsonrpc id is also passed-through, how does Ripple know which id space the response is from?
For example, if app1 makes a "methodA" call with jsonrpc id=1 and app2 makes a "methodB" call also with jsonrpc id=1 and both are sent through the passthrough. When the response comes back with id=1, how does Ripple know the response is meant for app1 or app2.

#### Option 1: Do not pass-through the jsonrpc id from the original client

Ripple should be a buffer between the app client and the implementing handler. Each are isolated jsonrpc messages with their own id space. So for the example above, the two calls from two different apps will result in their own jsonrpc id part of the incremented jsonrpc id for that connection to the passthrough provider.

#### Option 2: Connection per app

Just as each app has their own connection to Ripple, and thus has its own jsonrpc id space, Ripple can also have a connection to the provider for each app. Now when the response comes from the provider websocket, we know which app it is for and the ids can be passed through as well.
This also could have an added bonus of passing the context at connect time instead of on each individual message. The appId can be in the url, although that is probably not supported in Thunder to pass url context to the Thunder plugin?
This also would not allow included_context to be granular per rpc method.

### Validation of rpc arguments

If the message of the rpc method call is just a passthrough, should Ripple still do the argument validation?

#### Option 1: Ripple does the validation
Ripple can continue to do the validation, however currently the validation is done by the RPC handler when it deserialized the message into Rust structs. Passthrough providers should not reach Ripple handler code, and thus should not be deserialized.
Possibly validation can be done another way, through Ripple reading the RPC specification and checking each field.
One goal of the passthrough pattern is that code development in the Ripple core code should be near nothing, updating the rpc spec and configuring in the device manifest could be the only change that is made when adding a new passthrough API.

#### Option 2: The provider does the validation
The provider already will need to deserialize the message, and thus can do the validation there. For example, if the spec says a field is a string but the client passed a boolean, it should fail to deserialize on the provider.
Down side is that the provider now need to implement all the boiler plate errors in the same way that is expected for any firebolt APIs. Each provider may implement it differently and not to spec.