// Copyright 2025 Comcast Cable Communications Management, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;

use crate::{log::error, utils::error::RippleError};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RulesFunction {
    pub params: Option<Vec<String>>,
    pub body: String,
}

impl RulesFunction {
    pub fn execute(&self) -> String {
        String::new()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]

pub struct RulesImport {
    pub functions: HashMap<String, RulesFunction>,
}

fn build_param_list(params: &str) -> Vec<String> {
    let mut param_list = Vec::new();
    let mut current_param = String::new();
    let mut paren_pairs = 0;

    for character in params.chars() {
        if character == '(' {
            paren_pairs += 1;
        } else if character == ')' {
            paren_pairs -= 1;
        } else if character == ',' && paren_pairs == 0 {
            param_list.push(current_param.trim().to_string());
            current_param.clear();
            continue;
        }
        current_param.push(character);
    }

    if !current_param.is_empty() {
        param_list.push(current_param.trim().to_string());
    }

    param_list
}

pub fn apply_functions(
    input: &str,
    imports: &HashMap<String, RulesFunction>,
) -> Result<String, RippleError> {
    const FUNCTION_PREFIX: &str = "$function.";
    let mut output = input.to_string();

    // Locate function call in input string

    if let Some(index) = input.find(FUNCTION_PREFIX) {
        let function_start_index = index + FUNCTION_PREFIX.len();

        let mut function_end_index;

        if let Some(params_start_index) = input[function_start_index..].find('(') {
            function_end_index = function_start_index + params_start_index;
            let mut paren_pairs = 0;

            // Find the end of the function. Function params could include '(', and ')'.

            while function_end_index < input.len() {
                if input.chars().nth(function_end_index) == Some('(') {
                    paren_pairs += 1;
                } else if input.chars().nth(function_end_index) == Some(')') {
                    paren_pairs -= 1;
                }

                function_end_index += 1;

                if paren_pairs == 0 {
                    break;
                }
            }
        } else {
            error!("apply_functions: No opening parenthesis found for function call");
            return Err(RippleError::ParseError);
        }

        let function_call = input[function_start_index..function_end_index].to_string();

        if let Some(function_name_end) = function_call.find('(') {
            let function_name = &function_call[..function_name_end];
            let params = function_call[function_name_end + 1..function_call.len() - 1].to_string();

            // Gather parameters and perform replacements

            let param_list: Vec<String> = build_param_list(&params);

            if let Some(function) = imports.get(function_name) {
                let mut function_body = function.body.clone();

                if let Some(params) = &function.params {
                    for (i, param) in params.iter().enumerate() {
                        if i < param_list.len() {
                            let from = format!("${}", param);
                            let to = param_list[i].to_string();
                            function_body = function_body.replace(&from, &to);
                        } else {
                            error!(
                                "apply_functions: Not enough parameters provided for function {}",
                                function_name
                            );
                            return Err(RippleError::ParseError);
                        }
                    }
                }

                let function_token = format!("$function.{}", function_call);
                output = input.replace(&function_token, &function_body);
            } else {
                error!(
                    "apply_functions: Not found: function_name={}, imports={:?}",
                    function_name, imports
                );
                return Err(RippleError::ParseError);
            }
        }
    }

    if output.contains(FUNCTION_PREFIX) {
        // Recurse until all functions have been applied.
        output = apply_functions(&output, imports)?;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_param_list() {
        let params = "param1, param2, param3";
        let result = build_param_list(params);
        assert_eq!(result, vec!["param1", "param2", "param3"]);

        let params_with_nested = "param1, func(param2, param3), param4";
        let result = build_param_list(params_with_nested);
        assert_eq!(result, vec!["param1", "func(param2, param3)", "param4"]);
    }

    #[test]
    fn test_apply_functions_simple_replacement() {
        let mut imports = HashMap::new();
        imports.insert(
            "test_function".to_string(),
            RulesFunction {
                params: Some(vec!["param".to_string()]),
                body: "Hello, $param!".to_string(),
            },
        );

        let input = "$function.test_function(World)";
        let result = apply_functions(input, &imports).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_apply_functions_multiple_replacements() {
        let mut imports = HashMap::new();
        imports.insert(
            "test_function".to_string(),
            RulesFunction {
                params: Some(vec!["param1".to_string(), "param2".to_string()]),
                body: "$param1 and $param2".to_string(),
            },
        );

        let input = "$function.test_function(Hello, World)";
        let result = apply_functions(input, &imports).unwrap();
        assert_eq!(result, "Hello and World");
    }

    #[test]
    fn test_apply_functions_nested_function_calls() {
        let mut imports = HashMap::new();
        imports.insert(
            "inner_function".to_string(),
            RulesFunction {
                params: Some(vec!["param".to_string()]),
                body: "Inner $param".to_string(),
            },
        );
        imports.insert(
            "outer_function".to_string(),
            RulesFunction {
                params: Some(vec!["param".to_string()]),
                body: "Outer $param".to_string(),
            },
        );

        let input = "$function.outer_function($function.inner_function(World))";
        let result = apply_functions(input, &imports).unwrap();
        assert_eq!(result, "Outer Inner World");
    }

    #[test]
    fn test_apply_functions_missing_function() {
        let imports = HashMap::new();
        let input = "$function.non_existent_function()";
        let result = apply_functions(input, &imports);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_functions_insufficient_parameters() {
        let mut imports = HashMap::new();
        imports.insert(
            "test_function".to_string(),
            RulesFunction {
                params: Some(vec!["param1".to_string(), "param2".to_string()]),
                body: "$param1 and $param2".to_string(),
            },
        );

        let input = "$function.test_function(Hello)";
        let result = apply_functions(input, &imports);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_functions_no_parameters() {
        let mut imports = HashMap::new();
        imports.insert(
            "test_function".to_string(),
            RulesFunction {
                params: None,
                body: "No params needed".to_string(),
            },
        );

        let input = "$function.test_function()";
        let result = apply_functions(input, &imports).unwrap();
        assert_eq!(result, "No params needed");
    }
}
