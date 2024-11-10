#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_workflow_empty_sources() {
        let broker_request = BrokerRequest {
            rpc: RpcRequest {
                method: "test_method".to_string(),
                params_json: "[]".to_string(),
                ctx: Context::default(),
                stats: Stats::default(),
            },
            rule: Rule {
                sources: Some(vec![]),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = WorkflowBroker::run_workflow(&broker_request, &Context::default()).unwrap();
        assert!(result.result.is_none());
    }

    #[test]
    fn test_run_workflow_single_source() {
        let broker_request = BrokerRequest {
            rpc: RpcRequest {
                method: "test_method".to_string(),
                params_json: "[]".to_string(),
                ctx: Context::default(),
                stats: Stats::default(),
            },
            rule: Rule {
                sources: Some(vec![JsonDataSource {
                    method: "sub_method_1".to_string(),
                    params: None,
                    namespace: Some("ns1".to_string()),
                }]),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = WorkflowBroker::run_workflow(&broker_request, &Context::default()).unwrap();
        assert!(result.result.is_some());
    }

    #[test]
    fn test_run_workflow_multiple_sources() {
        let broker_request = BrokerRequest {
            rpc: RpcRequest {
                method: "test_method".to_string(),
                params_json: "[]".to_string(),
                ctx: Context::default(),
                stats: Stats::default(),
            },
            rule: Rule {
                sources: Some(vec![
                    JsonDataSource {
                        method: "sub_method_1".to_string(),
                        params: None,
                        namespace: Some("ns1".to_string()),
                    },
                    JsonDataSource {
                        method: "sub_method_2".to_string(),
                        params: None,
                        namespace: Some("ns2".to_string()),
                    },
                ]),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = WorkflowBroker::run_workflow(&broker_request, &Context::default()).unwrap();
        assert!(result.result.is_some());
    }

    #[test]
    fn test_run_workflow_source_failure() {
        let broker_request = BrokerRequest {
            rpc: RpcRequest {
                method: "test_method".to_string(),
                params_json: "[]".to_string(),
                ctx: Context::default(),
                stats: Stats::default(),
            },
            rule: Rule {
                sources: Some(vec![JsonDataSource {
                    method: "failing_method".to_string(),
                    params: None,
                    namespace: Some("ns1".to_string()),
                }]),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = WorkflowBroker::run_workflow(&broker_request, &Context::default()).unwrap_err();
        assert!(result.is::<Error>());
    }
}
