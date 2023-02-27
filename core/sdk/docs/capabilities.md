# What is a Ripple Capability?
Ripple Capability is similar to a firebolt capability which defines a component's ability to perform a concrete unit of work.

#### Difference between a Channel and Extension

Channel is an extension which holds a state during the lifetime of its operations. Good example of a Channel is the Thunder Thread which connects to the device. Channel stores a state of the connection in its lifetime. Extensions can be loaded into channel to make it more extensible

Extension is a one time use and recycle object, there is no state persisted it performs and operation and makes itself available for recycling.
For example For the Device there would be a thunder channel. There could be some Proprietary API(S) like AuthService which will be part of a private repository and added as an extension to the Thunder channel.



Below is anatomy of a Ripple capability

> ripple:[channel/extn]:[class]:[service]:[feature (optional)]

#### Decoding some Ripple Capabilities

Below capability means the given plugin offers a channel for Device Connection. Service used  for the device connection is Thunder.
>ripple:channel:device:thunder

Below Capability means the given plugin offers a channel for Data Governance. Service used for the feature is `somegovernance` service
>ripple:channel:data-governance:somegovernance

Below capability means the given plugin offers an extension for the Device Thunder Channel. It offers the auth thunder plugin implementation.
>ripple:extn:device:thunder:auth

Below capability means the given plugin offers a permission extension for the distributor. Name of the service is called `fireboltpermissions`.
>ripple:extn:distributor:permissions:fireboltpermissions

Below capability means the given plugin offers a JsonRpsee rpc extension for a service named badger
>ripple:extn:jsonrpsee:bridge

