# Extension Client
Extension client is at the heart of the Ripple architecture by providing a reliable and easy to use Inter Extension communication. During the course of the document the usage of the term `Client` will be a synonym to the Extn Client. Similarly term `Main` means the `Ripple Main` application which exists in `core/main`.

## Glossary
__*FFI*__ - [Foreign Functional Interface](https://en.wikipedia.org/wiki/Foreign_function_interface) is the mechanism used for dynamic link library.
__*ABI*__ - [Application Binary Interface](https://en.wikipedia.org/wiki/Application_binary_interface) is the interface between the dynamic link library and the loader
__*Crossbeam Channel*__ - [Rust library](https://docs.rs/crossbeam/latest/crossbeam/channel/index.html) which provides channels that can work across the loader and the dynamic link libraries.
__*Extension*__ - Ripple Dynamic link library which can be loaded during runtime and offer more extensive capabilities. 

## Overview
Core objective for the Extn client is to provide a reliable and robust communication channel between the  `Main` and its extensions. There are challenges when using Dynamic Linked libraries which needs to be carefully handled for memory, security and reliability. `Client` is built into the `core/sdk` for a better Software Delivery and Operational(SDO) performance.

Lets look a little deeper into the `Client`

### Inter Extension Communication(IEC)
![Inter Extension Communication](./images/IEC.jpeg)
Above image is a zoomed out view of Extn clients are connected between the `Main` and couple of extensions. So there are 3 components in this above image

a. `Main Application` - This is the `core/main` application which is the loader and also the entry point to the Ripple application

b. `Device Channel Extension` - Device channel which maintains a state of persistent connection with the underlying Device Interface.

c `Some Other Extension` - Either a proprietary or a device specific extension which resides outside the main application and loaded as a Dynamic Link Library

#### All roads lead to Ripple Main
Visible similarity between `a`, `b` and `c` is that they all get an instance of the `Client` and they are connected to `a` and not to each other directly. This is a really important detail for `enforcing security and standardizing the communication` across extensions. Every request goes through to the `Main` application for validation and forwarding. Once the validations are successful Main redirects the request to the intended Extn.
![Capability Not Permitted](./images/capnotpermitted.jpeg)

Above image is a very good example of how `Main` rejects any requests from `c` if it does not have the permission for that capability.

#### Response needs no stopovers
`Main` checks for permissions for any given request coming from an extension using [Extn Capabilities](./capabilities.md). Given the security and standardization criteria are already met `Main` adds a callback to the request before forwarding to the Request Processor in another extension. The Request processor can now directly respond back to the original caller without hopping back to Main

![Capability Permitted](./images/cappermitted.jpeg)

### Crossbeam Channel
Crossbeam provides a safe FFI bridge between Extensions and `Main` any communication between `Client` happens on this channel. It uses MPMC which expands to Multiple Producer and Multiple Consumer model. 

Each extension `Client` gets a Sender to call `Main` with requests. It also uses this same sender to send back the response if there is no callback supplied for a given request.

`Client` also gets a receiver which is used for Message processing. Implementation sections covers the reciver in a bit more detail.

### Extn Message

Extn Message provides the API content structure for the client. In simpler terms if `Client` is the speaker then `Message` is the language. Message for Client is heavily  structured in a way to handle crossing FFI boundary between extensions. Here is the structure of a ExtnMessage

```
pub struct ExtnMessage {
    pub id: String,
    pub requestor: ExtnCapability,
    pub target: ExtnCapability,
    pub payload: ExtnPayload,
    pub callback: Option<CSender<CExtnMessage>>,
}
```

Below table explains what each field means
| Field        | Type           | Note  |
| ------------- |-------------| -----|
| id     | String| Usually an UUID to identify a specific message |
| requestor      | ExtnCapability      | Looks something like `ripple:main:internal:rpc` when converted to String for a request coming from `Main`  |
| target | ExtnCapability      |    Something like `ripple:channel:device:info` for the device channel info request|
| payload| ExtnPayload| Payloads will be discussed in detail on the next section|
|callback|Crossbeam `Sender<CExtnMessage>` | Usually added by `Main` to the `target` to respond back to the `requestor`|

### Extn Payloads
There are 3 types of Extn Payloads

1. Request - Could be a Call request or a subscription request.
2. Response - Response for a Call Request.
3. Event - Object used by listeners of subscription request.

![Payload types](./images/ExtnMessageTypes.jpeg)

### Encoding/Decoding
For a given Message to travel between dynamic link libraries it needs to make sure that the data is ABI friendly. To achieve this feature ExtnClient has an encoder and decoder to pass the data safely and securely across crossbeam channels.
![Encoding Decoding](./images/enc_dec.jpeg)
Extn client uses a C friendly Structure called CExtnMessage to safely convert the ExtnMessage object for FFI transport.
Each Extn client has a encoder and decoder for sending and receiving messages.

![Client Bone](./images/client_backbone.jpeg)
Above diagram explains how 2 clients are binded within a crossbeam channel using encoder and decoder this is the backbone for the Extn client.
## Implementation
`Client` contains a `Sender` and `Processors`. 

`Sender` responsibilities remain mainly to `encode` the ExtnMessage and pass it to the target.
`Processor` on the other hand handles more sophisticated operations depending on the type of the payload.

![Client operations](./images/client_operations.jpeg)
Above image explains the operations possible within Extn client.
### Request Response Lifecycle

#### Registration

#### Identification

#### Delegation

Below diagram explains the list of steps for an Extn request to get a response. 
![Request Response Lifecycle](./images/request_response_lifecycle.jpeg)

1. Caller provides the `ExtnPayload` this payload has to implement `ExtnPayloadProvider` provided in the `core/sdk`
2. `Client` receives the payload and checks the `requestor`. 
If `Client` is inside a Extension library it will send the payload to `Sender`
If `Client` is in `Main` it will check for the requestor permissions. Once Permissions are satisfied it will forward the request to the Target extn client. 
### Event Subscription Lifecycle

## References
