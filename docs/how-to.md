#How to Ripple?



## How to Contract?

Contracts are the building blocks for the Ripple 2.0 ecosystem. Here is an [ADR](./adr/ripple_contracts.md) explaining Contracts in detail

### How to add a new Contract?

Contracts are available in 
>core/sdk/src/framework/ripple_contract.rs

Before adding a new contract check if a similar one already exists.
To add a new contract follow the `camelCase` naming convention.
Add enough information on top of the entry using Rust docs used in the earlier contracts.

### How to add a Request to a new Contract?

Most contracts are mapped to a Request structure through the implementation of the `ExtnPayloadProvider` trait.
Below is an example for the same

```
pub enum CustomRequest {
    Get,
    Set(bool)
}

impl ExtnPayloadProvider for CustomRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(r) => match r {
                ...snip
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Custom((
            self.clone(),
        )))
    }

    fn contract() -> RippleContract {
        RippleContract::Custom
    }
}
```


### How to add a Processor to a new Contract?

Once a request is mapped to a contract we can start implementing a processor for the contract. The processor follows similar pattern which requires implementation of couple of traits.
Here is the example.

```
#[derive(Debug)]
pub struct CustomProcessor {
    state: CloneableState,
    streamer: ExtnStreamer,
}

impl ExtnStreamProcessor for CustomProcessor {
    type S = CloneableState;
    type V = CustomRequest;
    fn get_state(&self) -> Self::S {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for CustomProcessor {
    async fn process_error(
        _state: Self::S,
        _msg: ExtnMessage,
        _error: ripple_sdk::utils::error::RippleError,
    ) -> Option<bool> {
            #... snip

    }

    async fn process_request(state: Self::S,
        msg: ExtnMessage,
        extracted_message: Self::V,
    ) -> Option<bool> {
            #... snip
    }


    client.addRequestProcessor(CustomProcessor)

```

### How to call a request using the new Contract?

Calling and receiving response is pretty straight forward.

```
 if let Ok(response) = client.request(CustomRequest::Get).await {
    if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
        // Success receiving a String response from the payload
    } else {
        // Error
    }
```

If calling from an extension we need to make sure the Extn manifest is updated accordingly as detailed [here](#how-to-update-extension-manifest).

## How to add a new Ripple Extension?

Create a new rust repo and setup the cargo.toml like below
```
[package]
name = "distributor_general"
version = "0.1.0"
edition = "2021"

# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[lib]
crate-type = ["cdylib"]

[dependencies]
# Ripple sdk should be opensourced and available from crates.io at this point
ripple_sdk = "0.6.0" 
serde_json = "1.0"
serde = { version = "1.0", features = ["derive"] }
```

Each extension should define the metadata and the builder.

### How to define the Metadata for the extension?
Each extension needs to provide the metadata for its usage and capabilities of the contracts.
Here is an example.

```

/// Defines the contracts fulfilled by this extension. This should match the extension manifest file
fn init_library() -> CExtnMetadata {
    let _ = init_logger("distributor_general".into());

    let dist_meta = ExtnSymbolMetadata::get(
        ExtnId::new_channel(ExtnClassId::Distributor, "general".into()),
        ContractFulfiller::new(vec![
            RippleContract::Permissions
        ]),
        Version::new(1, 1, 0),
    );

    debug!("Returning distributor builder");
    let extn_metadata = ExtnMetadata {
        name: "distributor_general".into(),
        symbols: vec![dist_meta],
    };
    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);
```

### How to build a Channel?
For a channel extension, Extension client needs to be initialized and processors needs to be setup as defined in the Contract Fulfiller.

Here is an example

```
fn start(sender: ExtnSender, receiver: CReceiver<CExtnMessage>) {
    let _ = init_logger("distributor_general".into());
    info!("Starting distributor channel");
    let runtime = Runtime::new().unwrap();
    let mut client = ExtnClient::new(receiver.clone(), sender);
    runtime.block_on(async move {
        let client_c = client.clone();
        tokio::spawn(async move {
            client.add_request_processor(DistributorPermissionProcessor::new(client.clone()));
            
            // Lets Main know that the distributor channel is ready
            let _ = client.event(ExtnStatus::Ready).await;
        });
        client_c.initialize().await;
    });
}

fn build(extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
    if let Ok(id) = ExtnId::try_from(extn_id.clone()) {
        let current_id = ExtnId::new_channel(ExtnClassId::Distributor, "general".into());

        if id.eq(&current_id) {
            return Ok(Box::new(ExtnChannel {
                start,
            }));
        } else {
            Err(RippleError::ExtnError)
        }
    } else {
        Err(RippleError::InvalidInput)
    }
}

fn init_extn_builder() -> ExtnChannelBuilder {
    ExtnChannelBuilder {
        build,
        service: "distributor_general".into(),
    }
}

export_channel_builder!(ExtnChannelBuilder, init_extn_builder);

```


### How to update extension Manifest

Extension manifest requires the name of the dynamic library and path if not relative.
Below is a snapshot of a extension manifest entry for this new extension

```
{
            "path": "libdistributor_general",
            "symbols": [
                {
                    "id": "ripple:channel:distributor:general",
                    "uses": [
                        "config"
                    ],
                    "fulfills": [
                        "permissions"
                    ]
                }
            ]
        }
```