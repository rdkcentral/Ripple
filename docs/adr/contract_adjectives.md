# Adding Adjectives to Contracts

## Status

__**Proposed**__

## Context

### Glossary
`Main`: Ripple `Main` application which starts the Ripple platform.

`Extension`(alias `Extn`): Runtime dynamically linked libraries built using Ripple SDK, `Main` loads these libraries during startup to offer enhanced capabilities.

`ExtnCapability` : Current implementation of identifying and encapsulating a function of a given extension. ExtnCapability has an anatomy of `ripple:[channel/extn]:[ExtnClass]:[service]`.

`ExtnClass` : Classifies a given capability by mapping it to a functional Ripple unit like Device, Launcher, Distributor etc.

`IEC`: Inter Extn Communication implemented through the `ExtnClient`.

`Contract`: A concrete unit of work expected to be available through extensions.


### Problems
Current Implementation of the Contractshave raised the following concerns.
For easier linking and references we are going to denote the problems with a `P: [A-Z](n)` notation. 
#### [P:A]  Horizontally growing Contracts
Just like `Common Nouns` some contracts have a common feature theme but are horizontally added into contracts.
These contracts increase the number of enumerations in the `RippleContracts`.

Number of contracts are growing(mostly horizontally) for a related feature.

| Feature        | Contract           | 
| ------------- |-------------|
|Storage |`PrivacyCloudStore` `UserGrantsCloudStore` `UserGrantsLocalStore` `PrivacySettingsLocalStore` `DevicePersistence`, `StorageManager`, `SecureStorage`|
|Metrics | `Metrics` `OperationalMetricListener` `BehaviorMetrics`|
|Session | `AccountSession` `SessionToken`|

This can continue to grow further for example
| Feature        | Contract           | 
| ------------- |-------------|
|Accessory|`RemoteAccesory`,`SpeakerAccessory`,`CameraAccessory`|

#### [P:B] Loaded Contracts causing Extension and processor exclusivity
Some contracts like `SessionToken` are loaded to restric processors to fulfill a common contract across Extensions. For eg `SessionToken` can only be implemented in one Extension and it has to provide all tokens for a given distributor. So if a distributor uses a Device Channel for one token, and a distributor channel for another token it cannot be done.

#### [P:C] Doesnt offer Processor and Request Resuability
Extn Request objects are also growing because an `ExtnRequestProcessor` usually offers support for only one `ExtnRequest`. These requests are similar in nature and processor should be able to offer multiple contracts.

#### [P:D] Lack of future readiness
Future Firebolt API versions would have Capabiliity Provider based API(s). If we need extensions we need a better way to identify the Contracts based on Firebolt Capability.


## Proposed Solution

### Noun Theory
A single Ripple Contract can be attributed to a `Noun` eventhough it refers to an action like a `Verb`. Contracts in Ripple world are always used in conjunction with encapsulation and identification of a processor provided by an Extension. So for the given argument let us consider `Ripple Contract` to a `Noun` based on its usage.

Let's see some attributes which relate them

`Proper Noun`: The name given to a particular person, place or thing usually starts with a capital letter.

`Proper Contract`: A contract which is singular in nature and cannot be split down any further. Like Wifi, Remote Control.

`Noun phrase`: Several words that, when grouped together, perform the same function of a noun.
Examples

1. I need a **toy**(common noun).

2. I need a **shiny toy**(noun phrase).

3. I need a **bouncy toy**(noun phrase).

The words **shiny** and **bouncy** used in above sentences are `Adjectives`, which decorated the common noun(toy) and provided a more deeper context.


Some Ripple contracts are similar to a common noun in the above example.
Let us apply some adjectives to P: A.

#### Storage
1. I want to store a **privacy local** value.
2. I want to store a **privacy cloud** value.
3. I want to store a **usergrant local** value.
4. I want to store a **usergrant cloud** value.
5. I want to store a **local device** value.

#### Metrics
1. I want to forward **behavior** metrics.
2. I want to forward **behavior** metrics.
3. I want to forward **operational** metrics.
4. I want to listen for **behavioral** metrics.
5. I want to listen for **operational** metrics.

#### Session
1. I want to get **Account Session** token.
2. I want to get **Platform** token.
3. I want to get **Device** token.
4. I want to get **Distributor** token.
5. I want to get **Root** token.

Contracts with `Adjective` add a crisper purpose for abstract Contracts to provide deeper meaning and reusability.

## Implementation

### Solving [P:A] Adding Adjectives for Storage Common Contract

To solve this problem we can treat `Storage` as a `common contract` similar to a `common noun`(**toy**) and add adjectives.

#### Step 1. Create a Contract Adjective trait

For a given Enumeration or Structure to become a Contract Adjective it should implement the `ContractAdjective` trait. Below is the definition of the `Contract Adjective` trait.

```
pub trait ContractAdjective : serde::ser::Serialize{
    fn as_string(&self) -> String {
        SerdeClearString::as_clear_string(self)
    }
    fn get_contract(&self) -> RippleContract;
}
```
`ContractAdjective` trait auto implements the `as_string()` and expects only the mapping contract from its implementors.

#### Step 2. Create Enumerations for Adjectives

```
pub enum StorageAdjective {
    PrivacyCloud,
    PrivacyLocal,
    UsergrantCloud,
    UsergrantLocal,
    Local,
    Manager,
    Secure
}

// Do the same for metrics and session
pub enum MetricContractAdjective{
    Forwarder,
    SendBehaviorMetrics,
    ListenOperationalMetrics
}

pub enum SesssionContractAdjective{
    Account,
    Platform,
    Device,
    Distributor,
    Root
}
```

#### Step 3. Implement Contract Adjective for Enumerations
Use SerdeClearString utility like Contracts.
```
impl ContractAdjective for StorageAdjective {
    fn get_contract(&self) -> RippleContract {
        RippleContract::Storage(self.clone())
    }
}

..snip Do the same for metrics and session
```

#### Step 4. Update Ripple Contracts

```
pub enum RippleContract {
... snip
--PrivacyCloudStore,
--UserGrantsCloudStore,
--UserGrantsLocalStore,
--PrivacySettingsLocalStore,
--DevicePersistence,
--StorageManager,
--SecureStorage,
++Storage(StorageAdjective)
... snip

... snip
same for Metrics and Session
... snip
}
```

Result of these changes offer a smaller set of enumerations in  `RippleContract` and more extensibility ergo solving [P:A].

### Solving [P:B] Adding Adjectives and solving Request to Extension exclusivity
Fortunately this problem is already accounted in the existing architecture just needs some tweaks for `Adjectives`

`ExtnPayloadProvider::get_contract()` already supports overriding of Contracts based on individual provider.

```
fn get_contract(&self) -> RippleContract {
        Self::contract()
}
```

By overriding the `get_contract()` method like so we can offer a more diverse adjectives for a given request.
```
fn get_contract(&self) -> RippleContract {
        match self.token_type {
            TokenType::Root => RippleContract::Session(SessionAdjective::Root),
            TokenType::Device => RippleContract::Session(SessionAdjective::Device),
            TokenType::Platform => RippleContract::Session(SessionAdjective::Platform),
            TokenType::Distributor => RippleContract::Session(SessionAdjective::Distributor)
        }
    }
```

### Solving [P:C] Making processor fulfill multiple contracts

First step to do this is to update SDK to provide support for same processor offering multiple contracts.

#### Step 1 Update ExtnStreamProcessor

First step is to add a default method called `fulfills_multiple`, as this would avoid changing existing set of processors already implemented

```
fn fulfills_mutiple(&self) -> Option<Vec<RippleContract>> {
        None
}
```
#### Step 2 Update Extn Client request processor

`ExtnClient` would now need to 
1. Check if a given processor fulfills multiple contract if so add mapping for each contract for that processor.
2. Use `contract.as_clear_string()` as key vs `contract.into()`, because the contract can have json structure due to serialization due to adjectives.

#### Step 3 Update example distributor
With `SessionTokenRequest` as example we can now update the `general_token_processor` fulfill multiple Contracts given the request object is same.

```
fn supports_mutiple(&self) -> Option<Vec<ripple_sdk::framework::ripple_contract::RippleContract>> {
        Some(vec![RippleContract::Session(SessionAdjective::Root), RippleContract::Session(SessionAdjective::Device)])
    }
```

Now `ExtnRequest` objects are reusable, and processors are reusable for same request object.

### Solving [P:D] Making FireboltCap as an Adjective for FireboltProvider Contracts

Near future would hold a new contract called `FireboltProvider`. This would be an extension which can provide a `FireboltCap` as part of its execution. This Extension would connect back to Websocket using the same gateway as Apps.

With this expectation we can reuse this architecture and offer `FireboltCap` as a `ContractAdjective`.
Manifest entries for the same would look like this.
```
../ ripple_contract.rs
impl ContractAdjective for FireboltCap{
    ..snip
}

pub enum RippleContract{
    ...
    FireboltProvider(FireboltCap)
}

```

Now we can create extensions which can fulfill a Capability for a given Extension

Here is an example

#### Manifest Entry
`voice:alexa.firebolt_provider`

#### FFI.rs entry

```
...
 ContractFulfiller::new(vec![RippleContract::FireboltProvider(Fireboltcap::Short("voice:alexa"))
 ...
```

This architecture would make the `FireboltProvider` Contract more impactful for `ProviderBroker` to have better discovery of the capabilities.