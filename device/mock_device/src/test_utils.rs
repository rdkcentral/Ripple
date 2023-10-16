use std::collections::HashMap;

use ripple_sdk::{
    crossbeam::channel::{unbounded, Receiver},
    extn::{
        client::extn_sender::ExtnSender,
        extn_id::{ExtnClassId, ExtnId},
        ffi::ffi_message::CExtnMessage,
    },
};

pub fn extn_sender_web_socket_mock_server() -> (ExtnSender, Receiver<CExtnMessage>) {
    let (tx, receiver) = unbounded();
    let sender = ExtnSender::new(
        tx,
        ExtnId::new_channel(ExtnClassId::Device, "mock_device".to_owned()),
        vec![],
        vec!["web_socket.mock_server".to_owned()],
        Some(HashMap::from([(
            "mock_data_file".to_owned(),
            "examples/device-mock-data/mock-device.json".to_owned(),
        )])),
    );

    (sender, receiver)
}

// pub fn extn_sender_jsonrpsee() -> (ExtnSender, Receiver<CExtnMessage>) {
//     let (tx, receiver) = unbounded();
//     let sender = ExtnSender::new(
//         tx,
//         ExtnId::new_channel(ExtnClassId::Device, "mock_device".to_owned()),
//         vec!["web_socket.mock_server".to_owned()],
//         vec!["json_rpsee".to_owned()],
//         None,
//     );

//     (sender, receiver)
// }
