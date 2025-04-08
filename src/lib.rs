mod context;

use std::collections::{btree_map::Values, HashMap};

pub use context::*;

static STRING_DELIMITERS: [char; 4] = ['"', '\'', '“', '”'];
static NUMBER_CHARS: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '.', 'e', 'E', '/', ',',
];

struct True;
struct False;
struct Null;

#[cfg_attr(test, derive(Debug, PartialEq))]
enum JsonNumber {
    Integer(i64),
    Float(f64),
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum JsonValue {
    True,
    False,
    Null,
    String(String),
    Number(JsonNumber),
    Object(HashMap<String, JsonValue>),
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum BoolsOrNull {
    True,
    False,
    Null,
}
impl Into<JsonValue> for BoolsOrNull {
    fn into(self) -> JsonValue {
        match self {
            BoolsOrNull::True => JsonValue::True,
            BoolsOrNull::False => JsonValue::False,
            BoolsOrNull::Null => JsonValue::Null,
        }
    }
}
impl ToString for BoolsOrNull {
    fn to_string(&self) -> String {
        match self {
            BoolsOrNull::True => "true".to_string(),
            BoolsOrNull::False => "false".to_string(),
            BoolsOrNull::Null => "null".to_string(),
        }
    }
}

struct JSONParser {
    index: usize,
    json_str: Vec<char>,
    context: JsonContext,
}

impl JSONParser {
    pub fn new(json_str: String) -> Self {
        let json_str = json_str.chars().collect();
        JSONParser {
            index: 0,
            json_str,
            context: JsonContext::new(),
        }
    }

    pub fn parse_json(&mut self) -> Option<JsonValue> {
        loop {
            let char = match self.get_char_at(None) {
                Some(c) => c,
                None => break,
            };

            if char == '{' {
                self.index += 1;
                return self.parse_object();
            }
        }

        unimplemented!();
    }

    fn get_char_at(&self, count: Option<usize>) -> Option<char> {
        let index = count.unwrap_or(0) + self.index;
        self.json_str.get(index).copied()
    }

    fn parse_object(&mut self) -> Option<JsonValue> {
        let mut obj: HashMap<String, JsonValue> = HashMap::new();

        loop {
            // (self.get_char_at() or "}") != "}":
            let char = self.get_char_at(None).unwrap_or('}');
            if char == '}' {
                break;
            }

            self.skip_whitespaces_at(None, None);

            // Sometimes LLMs do weird things, if we find a ":" so early, we'll change it to "," and move on
            if self.get_char_at(None).map(|c| c == ':').unwrap_or(false) {
                self.log("While parsing an object we found a : before a key, ignoring");
                self.index += 1
            }

            // We are now searching for they string key
            // Context is used in the string parser to manage the lack of quotes
            self.context.set(ContextValues::ObjectKey);

            // Save this index in case we need find a duplicate key
            let mut rollback_index = self.index;

            // This probably could be a reference of str
            // TODO: check it
            let mut key = "".to_string();
            while self.get_char_at(None).is_some() {
                rollback_index = self.index;

                key = match self.parse_string() {
                    None => {
                        self.skip_whitespaces_at(None, None);
                        "".to_string()
                    }
                    Some(key) => key,
                };
                if key != ""
                    || (key == ""
                        && self
                            .get_char_at(None)
                            .map(|c| matches!(c, '}' | ':'))
                            .unwrap_or(false))
                {
                    // If the string is empty but there is a object divider, we are done here
                    break;
                }
            }

            // https://github.com/mangiucugna/json_repair/blob/5b57d4724a661eceb4415bdb39e5e48e87676263/src/json_repair/json_parser.py#L147
            if self.context.context.contains(&ContextValues::Array) && obj.contains_key(&key) {
                self.log(
                    "While parsing an object we found a duplicate key, closing the object here and rolling back the index",
                );
                self.index = rollback_index - 1;
                // add an opening curly brace to make this work

                self.json_str.insert(self.index + 1, '{');
                break;
            }

            // https://github.com/mangiucugna/json_repair/blob/5b57d4724a661eceb4415bdb39e5e48e87676263/src/json_repair/json_parser.py#L159
            // START FROM HERE NEXT TIME
        }

        self.index += 1;
        return panic!();
    }

    fn parse_string(&mut self) -> Option<String> {
        // <string> is a string of valid characters enclosed in quotes
        // i.e. { name: "John" }
        // Somehow all weird cases in an invalid JSON happen to be resolved in this function, so be careful here

        let mut missing_quotes = false;
        let mut doubled_quotes = false;
        let mut lstring_delimiter = '"';
        let mut rstring_delimiter = '"';

        let mut char = self.get_char_at(None);
        if matches!(char, Some('#') | Some('/')) {
            return self.parse_comment();
        }

        while char
            .map(|c| !STRING_DELIMITERS.contains(&c) && c.is_alphanumeric())
            .unwrap_or(false)
        {
            self.index += 1;
            char = self.get_char_at(None);
        }

        let char = match char {
            Some(c) => c,
            None => {
                return None;
            }
        };

        // Ensuring we use the right delimiter
        if char == '\'' {
            lstring_delimiter = '\'';
            rstring_delimiter = '\'';
        } else if char == '“' {
            lstring_delimiter = '“';
            rstring_delimiter = '”';
        } else if char.is_alphanumeric() {
            // This could be a <boolean> and not a string. Because (T)rue or (F)alse or (N)ull are valid
            // But remember, object keys are only of type string
            if ['t', 'f', 'n'].contains(&char.to_lowercase().next().unwrap())
                || self
                    .context
                    .current
                    .map(|c| c != ContextValues::ObjectKey)
                    .unwrap_or(true)
            {
                let value = self.parse_boolean_or_null();
                if let Some(value) = value {
                    return Some(value.to_string());
                }
            }
            self.log("While parsing a string, we found a literal instead of a quote");
            missing_quotes = true;
        }

        if !missing_quotes {
            self.index += 1;
        };

        // There is sometimes a weird case of doubled quotes, we manage this also later in the while loop
        if self
            .get_char_at(None)
            .map_or(false, |c| STRING_DELIMITERS.contains(&c))
        {
            // If the next character is the same type of quote, then we manage it as double quotes
            if self.get_char_at(None) == Some(lstring_delimiter) {
                // If it's an empty key, this was easy

                if self.context.current == Some(ContextValues::ObjectKey)
                    && self.get_char_at(Some(1)) == Some(':')
                {
                    self.index += 1;
                    return None;
                }

                if self.get_char_at(Some(1)) == Some(lstring_delimiter) {
                    // There's something fishy about this, we found doubled quotes and then again quotes
                    self.log(
                        "While parsing a string, we found a doubled quote and then a quote again, ignoring it",
                    );
                    return None;
                }

                // Find the next delimiter
                let i = self.skip_to_character(rstring_delimiter, 1);

                // https://github.com/mangiucugna/json_repair/blob/main/src/json_repair/json_parser.py#L295
            }
        }

        None
    }

    // https://github.com/mangiucugna/json_repair/blob/5b57d4724a661eceb4415bdb39e5e48e87676263/src/json_repair/json_parser.py#L657C5-L657C70
    fn parse_number(&mut self) -> Option<JsonValue> {
        // <number> is a valid real number expressed in one of a number of given formats
        let mut number_str = String::new();
        let mut char = self.get_char_at(None);

        let is_array = self.context.current == Some(ContextValues::Array);

        while char.map_or(false, |c| {
            NUMBER_CHARS.contains(&c) && (!is_array || &c != &',')
        }) {
            number_str += &char.unwrap().to_string();
            self.index += 1;
            char = self.get_char_at(None);
        }

        let number_str_trimmed = number_str
            .trim_end_matches(&['e', 'E', '-', '/', ','])
            .to_string();

        if number_str_trimmed.len() != number_str.len() {
            self.index -= 1;
        }

        if number_str_trimmed.contains(',') {
            return Some(JsonValue::String(number_str_trimmed));
        }

        let number_chars = ['e', 'E', '.'];
        if number_chars.iter().any(|c| number_str_trimmed.contains(*c)) {
            match number_str_trimmed.parse::<f64>() {
                Ok(f) => return Some(JsonValue::Number(JsonNumber::Float(f))),
                Err(_) => {
                    self.log("While parsing a number, we found a float that is not a float");
                    return None;
                }
            }
        } else if number_str_trimmed == "-" {
            return self.parse_json();
        } else {
            match number_str_trimmed.parse::<i64>() {
                Ok(i) => return Some(JsonValue::Number(JsonNumber::Integer(i))),
                Err(_) => {
                    self.log("While parsing a number, we found an integer that is not an integer");
                    return None;
                }
            }
        }
    }

    fn skip_to_character(&mut self, closing_char: char, mut idx: usize) -> usize {
        let mut char = match self.json_str.get(self.index + idx) {
            Some(c) => *c,
            None => return idx,
        };

        while char != closing_char {
            idx += 1;

            char = match self.json_str.get(self.index + idx) {
                Some(c) => *c,
                None => return idx,
            };
        }

        if self.index + idx > 0 && self.json_str[self.index + idx - 1] == '\\' {
            // Ah this is an escaped character, let's continue
            return self.skip_to_character(closing_char, idx + 1);
        }

        idx
    }

    fn parse_boolean_or_null(&mut self) -> Option<BoolsOrNull> {
        // <boolean> is one of the literal strings 'true', 'false', or 'null' (unquoted)

        let false_str = "false".chars().collect::<Vec<_>>();
        if self.json_str[self.index..(self.index + false_str.len())] == false_str {
            self.index += false_str.len();
            return Some(BoolsOrNull::False);
        }

        let true_str = "true".chars().collect::<Vec<_>>();
        if self.json_str[self.index..(self.index + true_str.len())] == true_str {
            self.index += true_str.len();
            return Some(BoolsOrNull::True);
        }

        let null_str = "null".chars().collect::<Vec<_>>();
        if self.json_str[self.index..(self.index + null_str.len())] == null_str {
            self.index += null_str.len();
            return Some(BoolsOrNull::Null);
        }

        return None;
    }

    fn parse_comment(&mut self) -> Option<String> {
        unimplemented!("parse_comment");
        None
    }

    fn log(&self, message: &str) {
        println!("{}: {}", self.index, message);
    }

    fn skip_whitespaces_at(&mut self, idx: Option<usize>, move_main_index: Option<bool>) -> usize {
        let mut idx = idx.unwrap_or(0);
        let move_main_index = move_main_index.unwrap_or(true);

        let mut char = self.json_str[self.index + idx];
        while char.is_whitespace() {
            if move_main_index {
                self.index += 1
            } else {
                idx += 1
            }
            char = match self.json_str.get(self.index + idx) {
                Some(c) => *c,
                None => return idx,
            }
        }

        idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_number() {
        let mut parser = JSONParser::new("123".to_string());
        let value = parser.parse_number();
        assert_eq!(value, Some(JsonValue::Number(JsonNumber::Integer(123))));
    }

    #[test]
    fn test_parse_number_float() {
        let mut parser = JSONParser::new("123.456".to_string());
        let value = parser.parse_number();
        assert_eq!(value, Some(JsonValue::Number(JsonNumber::Float(123.456))));
    }

    #[test]
    fn test_parse_number_float_with_e() {
        let mut parser = JSONParser::new("123.456e-7".to_string());
        let value = parser.parse_number();
        assert_eq!(
            value,
            Some(JsonValue::Number(JsonNumber::Float(123.456e-7)))
        );
    }

    #[test]
    fn test_parse_number_float_with_E() {
        let mut parser = JSONParser::new("123.456E-7".to_string());
        let value = parser.parse_number();
        assert_eq!(
            value,
            Some(JsonValue::Number(JsonNumber::Float(123.456e-7)))
        );
    }

    #[test]
    fn test_parse_number_float_with_slash() {
        let mut parser = JSONParser::new("123.456/".to_string());
        let value = parser.parse_number();
        assert_eq!(value, Some(JsonValue::Number(JsonNumber::Float(123.456))));
    }

    #[test]
    fn test_parse_number_float_with_comma() {
        let mut parser = JSONParser::new("123.456,".to_string());
        let value = parser.parse_number();
        assert_eq!(value, Some(JsonValue::Number(JsonNumber::Float(123.456))));
    }

    #[test]
    fn test_parse_number_negative() {
        let mut parser = JSONParser::new("-123.456".to_string());
        let value = parser.parse_number();
        assert_eq!(value, Some(JsonValue::Number(JsonNumber::Float(-123.456))));
    }

    #[test]
    fn test_parse_number_negative_integer() {
        let mut parser = JSONParser::new("-123".to_string());
        let value = parser.parse_number();
        assert_eq!(value, Some(JsonValue::Number(JsonNumber::Integer(-123))));
    }

    #[test]
    fn test_parse_number_negative_float_with_e() {
        let mut parser = JSONParser::new("-123.4e93".to_string());
        let value = parser.parse_number();
        assert_eq!(value, Some(JsonValue::Number(JsonNumber::Float(-123.4e93))));
    }
}
