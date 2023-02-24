# Extension Client
Extension client is at the heart of the Ripple architecture by providing a reliable and easy to use Inter Extension communication. During the course of the document the usage of the term `Client` will be a synonym to the Extn Client. Similarly term `Main` means the `Ripple Main` application which exists in `core/main`.

## Glossary
__*FFI*__ - [Foreign Functional Interface](https://en.wikipedia.org/wiki/Foreign_function_interface) is the mechanism used for dynamic link library.
__*ABI*__ - [Application Binary Interface](https://en.wikipedia.org/wiki/Application_binary_interface) is the interface between the dynamic link library and the loader
__*Crossbeam Channel*__ - [Rust library](https://docs.rs/crossbeam/latest/crossbeam/channel/index.html) which provides channels that can work across the loader and the dynamic link libraries.
__*Extension*__ - Ripple Dynamic link library which can be loaded during runtime and offer more extensive capabilities. 

## Overview
Main objective for the Extn client is to provide a reliable and robust communication channel between the  Ripple main applications and its extensions. There are challenges when using Dynamic Linked libraries which needs to be carefully handled for memory, security and reliability. `Client` is built into the Ripple core sdk for a better Software delivery and Operational(SDO) performance.

Lets look into how the `Client` works

### Inter Extension Communication(IEC)
![Inter Extension Communication](./images/IEC.jpeg)
Above image is a zoomed out view of Extn clients are connected to each other. So there are 3 components in this above image
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

### Extn Message
### Encoding/Decoding

## Implementation

### Request Response Lifecycle

### Event Subscription Lifecycle

## References
