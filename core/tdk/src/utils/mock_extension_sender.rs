use async_channel::Receiver as CReceiver;
use ripple_sdk::extn::{
    client::extn_sender::ExtnSender, extn_id::ExtnId, ffi::ffi_message::CExtnMessage,
};
use std::collections::HashMap;

pub trait MockExtnSender {
    fn mock() -> (ExtnSender, CReceiver<CExtnMessage>)
    where
        Self: Sized;
    fn mock_with_params(
        id: ExtnId,
        context: Vec<String>,
        fulfills: Vec<String>,
        config: Option<HashMap<String, String>>,
    ) -> (ExtnSender, CReceiver<CExtnMessage>)
    where
        Self: Sized;
}

// Implement ExtnSenderMockable for ExtnSender
impl MockExtnSender for ExtnSender {
    fn mock() -> (ExtnSender, CReceiver<CExtnMessage>) {
        MockExtnSenderBuilder::new().build()
    }

    fn mock_with_params(
        id: ExtnId,
        context: Vec<String>,
        fulfills: Vec<String>,
        config: Option<HashMap<String, String>>,
    ) -> (ExtnSender, CReceiver<CExtnMessage>) {
        MockExtnSenderBuilder::new()
            .id(id)
            .context(context)
            .fulfills(fulfills)
            .config(config)
            .build()
    }
}

pub struct MockExtnSenderBuilder {
    id: ExtnId,
    context: Vec<String>,
    fulfills: Vec<String>,
    config: Option<HashMap<String, String>>,
}

impl MockExtnSenderBuilder {
    fn new() -> Self {
        MockExtnSenderBuilder {
            id: ExtnId::get_main_target("main".into()),
            context: vec!["context".to_string()],
            fulfills: vec!["fulfills".to_string()],
            config: Some(HashMap::new()),
        }
    }

    fn id(mut self, id: ExtnId) -> Self {
        self.id = id;
        self
    }

    fn context(mut self, context: Vec<String>) -> Self {
        self.context = context;
        self
    }

    fn fulfills(mut self, fulfills: Vec<String>) -> Self {
        self.fulfills = fulfills;
        self
    }

    fn config(mut self, config: Option<HashMap<String, String>>) -> Self {
        self.config = config;
        self
    }

    fn build(self) -> (ExtnSender, CReceiver<CExtnMessage>) {
        let (tx, rx) = async_channel::unbounded();
        (
            ExtnSender {
                tx,
                id: self.id,
                permitted: self.context,
                fulfills: self.fulfills,
                config: self.config,
            },
            rx,
        )
    }
}
