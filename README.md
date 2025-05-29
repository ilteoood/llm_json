# LLM_JSON

A Rust library to repair broken JSON strings, particularly useful for handling malformed JSON output from Large Language Models.

This is a porting of the Python library [json_repair](https://github.com/mangiucugna/json_repair), written by [Stefano Baccianella](https://github.com/mangiucugna) and published under the MIT license.

All credits go to the original author for the amazing work.

## Usage

```sh
cargo add llm_json
```

```rust
use llm_json::{repair_json, loads, JsonRepairError};

// Basic repair
let broken_json = r#"{name: 'John', age: 30,}"#;
let repaired = repair_json(broken_json, &Default::default())?;
println!("{}", repaired); // {"name": "John", "age": 30}

// Parse directly to Value
let value = loads(broken_json, &Default::default())?;
```

## License

[MIT](/LICENSE.md)