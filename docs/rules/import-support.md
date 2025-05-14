<div align="center">
<h1>JQ Rules Function Support</h1>
</div>

<br>
<h2>Overview</h2>
The Ripple JQ Rules Engine allows for the definition of rule "functions", offering a way to centralize common JQ rule patterns. Rule functions can greatly improve readability, reduce time spent maintaining rule files, and help prevent bugs. Like many scripting languages, rule functions allow for zero or more parameters in order to customize the function output.

<h2>Rule Definition</h2>

A rule function, defined in JSON, has the following format as imagined in rust:

```
pub fn RulesFunction {
    pub params: Option<Vec<String>>,
    pub body: String,
}
```

...where `params` is an optional list of function parameters and `body` is the output of the function. `body` typically contains a JQ rule but in reality rule functions can return any string.

Rule functions are defined in their own JSON file as such:

```
"functions": {

    "territory_firebolt2thunder": {
        "params": [
            "territory",
            "default"
        ],
        "body": "if $territory == \"US\" then { \"territory\": \"USA\" } else { \"territory\": $default } end"
    },

    "return_or_error": {
        "params": [
            "return_val",
            "error_message"
        ],
        "body": "if .error or (.result and .result.success and .result.success != true) then { error: { code: -32100, message: $error_message }} else ($return_val) end"
    }
}
```

In this example there are two functions, `territory_firebolt2thunder` and `return_or_error`. The first function, `territory_firebolt2thunder`, converts the territory specified in firebolt format to the format expected by thunder. If the specified territory cannot be mapped, the territory indicated by the `default` parameter is used. The rust analog for the first function would be:

```
pub fn territory_firebolt2thunder(territory: String, default: String) -> String
```

Parameters can be referenced in the function body by prefixing with `$`. For example, the parameter `territory` is referenced in the body as `$territory`, similar to how contextual variables are defined e.g. `$context.app_id`.

The second function, `return_or_error`, handles common functionality found in a large number of Firebolt rules and is a good example of how functions can help reduce boilerplate result transforms in rules files. This function checks the thunder result and, if successful, returns the specified `return_val`; otherwise it returns an error with the message specified by `error_message`.

The rust analog for the second function would be:

```
pub fn return_or_error(return_val: String, error_message: String) -> String
```

<br>
<h2>Importing Function Definition Files</h2>

Function definition files are imported into a rules file as such:
```
{
    "imports": [
        "/etc/ripple/rules/rule_utils.json"
    ],
    .
    .
    .
```

...where `imports` is an array or one or more JSON files in the format described in [Rule Definition](#rule-definition). As currently designed, any similarly-named functions in subsequent import files would overwrite those defined in previously-imported files. In other words, array order dictates override behavior similar to how subsequently defined rules override those previously defined.
<br>
<h2>Function Usage</h2>

After creating and importing a function definition file, defined functions can be called within the rules file, for example:

```
"localization.setCountryCode": {
    "alias": "org.rdk.System.setTerritory",
    "transform": {
        "request": "$function.territory_firebolt2thunder(.value, \"USA\")",
        "response": "$function.return_or_error(null, \"couldn't set countrycode\")"s
    }
}
```

Here, functions are used for both the `request` and `response` transforms. In the first case, `.value` is passed into `territory_firebolt2thunder`. If `.value` equalled "AK", after function execution the `request` transform would become:

```
"request": "\"USA\""
```

