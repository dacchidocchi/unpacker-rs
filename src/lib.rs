//! A utility for unpacking P.A.C.K.E.R. encoded JavaScript code.
//!
//! This unpacker **restores** the original code from P.A.C.K.E.R. compressed/obfuscated JavaScript.
//!
//! ## Typical P.A.C.K.E.R. Structure
//! ```javascript
//! eval(function(p,a,c,k,e,r){
//!   // Unpacking logic
//! }('payload', radix, count, 'symbol|table'.split('|'), 0, {}))
//! ```
//!
//! # Examples
//!
//! ```rust
//! let packed_code = r#"eval(function(p,a,c,k,e,r){...}('0 2=1',62,3,'var||a'.split('|'),0,{}))"#;
//!
//! // Unpack to original JavaScript
//! let original = unpacker_rs::unpack(&packed_code).unwrap();
//! assert_eq!(original, "var a=1");
//! ```

use std::sync::LazyLock;

use regex::Regex;

use crate::unbaser::Unbaser;

mod unbaser;

static PACKED_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"eval[ ]*\([ ]*function[ ]*\([ ]*p[ ]*,[ ]*a[ ]*,[ ]*c[ ]*,[ ]*k[ ]*,[ ]*e[ ]*,[ ]*",
    )
    .unwrap()
});

/// Detects whether the input string contains P.A.C.K.E.R. encoded JavaScript.
///
/// This method looks for the characteristic `eval(function(p,a,c,k,e,` pattern
/// that indicates P.A.C.K.E.R. encoding.
///
/// # Examples
///
/// ```rust
/// assert!(unpacker_rs::detect("eval(function(p,a,c,k,e,r){...}"));
/// assert!(!unpacker_rs::detect("var x = 1;"));
/// assert!(!unpacker_rs::detect(""));
/// ```
#[inline]
pub fn detect(source: &str) -> bool {
    PACKED_REGEX.is_match(source)
}

/// Unpacks P.A.C.K.E.R. encoded JavaScript code back to its original form.
///
/// This is the main entry point for the unpacking process. It first validates
/// that the input is P.A.C.K.E.R. encoded, then calls [`unpack_unchecked`] to
/// perform the actual unpacking.
///
/// # Errors
///
/// Returns an error if:
/// - The input is not P.A.C.K.E.R. encoded
/// - The symbol table count doesn't match the actual symbols
/// - The radix is unsupported by the internal `Unbaser`
/// - The code structure is malformed or unexpected
///
/// # Examples
///
/// ```rust
/// let packed = "eval(function(p,a,c,k,e,r){...}('0 2=1',62,3,'var||a'.split('|'),0,{}))";
/// let unpacked = unpacker_rs::unpack(packed).unwrap();
/// assert_eq!(unpacked, "var a=1");
/// ```
#[inline]
pub fn unpack(source: &str) -> Result<String, String> {
    if !detect(source) {
        return Err("Invalid p.a.c.k.e.r data.".to_string());
    }
    unpack_unchecked(source)
}

/// Unpacks P.A.C.K.E.R. encoded JavaScript code without validation.
///
/// This method performs the actual unpacking process without checking if the
/// input is P.A.C.K.E.R. encoded. Use this when you've already validated the
/// input with [`detect`] to avoid double validation.
///
/// # Errors
///
/// Returns an error if:
/// - The symbol table count doesn't match the actual symbols
/// - The radix is unsupported by the internal `Unbaser`
/// - The code structure is malformed or unexpected
///
/// # Safety
///
/// This function assumes the input has been validated with [`detect`].
/// Calling this on non-P.A.C.K.E.R. encoded input may result in unexpected
/// behavior or panics.
///
/// # Examples
///
/// ```rust
/// let packed = "eval(function(p,a,c,k,e,r){...}('0 2=1',62,3,'var||a'.split('|'),0,{}))";
///
/// // Safe usage: validate first
/// if unpacker_rs::detect(packed) {
///     let unpacked = unpacker_rs::unpack_unchecked(packed).unwrap();
///     assert_eq!(unpacked, "var a=1");
/// }
/// ```
pub fn unpack_unchecked(source: &str) -> Result<String, String> {
    let matcher = PACKED_REGEX.find(source).unwrap();
    let begin_offset = matcher.start();
    let begin_string = &source[..begin_offset];
    let end_string = ["')))", "}))"]
        .iter()
        .find_map(|delimiter| source.split_once(delimiter).map(|(_, end)| end))
        .unwrap_or("");

    let (payload, symtab, radix, count) = filter_args(source)?;

    if count != symtab.len() {
        return Err(format!(
            "Malformed p.a.c.k.e.r. symtab. ({} != {})",
            count,
            symtab.len()
        ));
    }

    let unbaser = Unbaser::new(radix)?;
    let new_source = decode_words(&payload, &symtab, &unbaser);
    let processed_source = replace_strings(&new_source);

    Ok(format!(
        "{}{}{}",
        begin_string, processed_source, end_string
    ))
}

/// Decodes placeholder words in the payload using the symbol table.
///
/// This function processes the compressed payload by:
/// 1. Cleaning escape sequences (`\\` → `\`, `\'` → `'`)
/// 2. Finding all word boundaries (`\b\w+\b`)
/// 3. Converting each word from the specified base to a decimal index
/// 4. Looking up the index in the symbol table
/// 5. Replacing valid placeholders with their original words
///
/// Words that can't be decoded or don't have valid symbol table entries
/// are left unchanged.
///
/// # Arguments
///
/// * `payload` - The compressed code with placeholder words
/// * `symtab` - Symbol table containing original words
/// * `unbaser` - Base converter for decoding placeholder indices
///
/// # Returns
///
/// The payload with placeholders replaced by original words
fn decode_words(payload: &str, symtab: &[String], unbaser: &Unbaser) -> String {
    static WORD_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\w+\b").unwrap());

    let cleaned = payload.replace(r"\\", r"\").replace(r"\'", "'");
    WORD_REGEX
        .replace_all(&cleaned, |caps: &regex::Captures| {
            let word = &caps[0];
            match unbaser.unbase(word) {
                Ok(index) if index < symtab.len() && !symtab[index].is_empty() => {
                    symtab[index].clone()
                }
                _ => word.to_owned(),
            }
        })
        .into_owned()
}

/// Extracts the P.A.C.K.E.R. arguments from the source code.
///
/// This function handles two P.A.C.K.E.R. patterns:
/// 1. Full pattern with extra parameters: `}('payload', radix, count, 'symbols'.split('|'), extra, params))`
/// 2. Simple pattern: `}('payload', radix, count, 'symbols'.split('|')`
///
/// # Returns
///
/// A tuple containing `(payload, symbol_table, radix, count)` where:
/// - `payload`: The compressed code string
/// - `symbol_table`: The symbol table split from pipe-separated string
/// - `radix`: Base used for encoding (e.g., 36, 62)
/// - `count`: Expected number of symbols
///
/// # Errors
///
/// Returns an error if:
/// - Neither pattern matches the source
/// - The radix or count cannot be parsed as numbers
/// - The code structure is unexpected
fn filter_args(source: &str) -> Result<(String, Vec<String>, usize, usize), &'static str> {
    static JUICERS: LazyLock<[Regex; 2]> = LazyLock::new(|| {
        [
            // Full pattern with additional parameters
            Regex::new(
                r"}\('(.*)', *(\d+|\[\]), *(\d+), *'(.*)'\.split\('\|'\), *(\d+), *(.*)\)\)",
            )
            .unwrap(),
            // Simple pattern without extra parameters
            Regex::new(r"}\('(.*)', *(\d+|\[\]), *(\d+), *'(.*)'\.split\('\|'\)").unwrap(),
        ]
    });

    for juicer in JUICERS.iter() {
        if let Some(caps) = juicer.captures(source) {
            let payload = caps[1].to_owned();
            let radix = match &caps[2] {
                "[]" => 62,
                radix_str => radix_str.parse().map_err(|_| "Invalid radix")?,
            };
            let count = caps[3].parse().map_err(|_| "Invalid count")?;
            let symtab = caps[4].split('|').map(String::from).collect();

            return Ok((payload, symtab, radix, count));
        }
    }

    Err("Could not make sense of p.a.c.k.e.r data (unexpected code structure)")
}

/// Replaces string array references with their actual string values.
///
/// Some P.A.C.K.E.R. variants create string arrays like:
/// ```javascript
/// var _0x1234=["string1","string2","string3"];
/// // Later referenced as: _0x1234[0], _0x1234[1], _0x1234[2]
/// ```
///
/// This function:
/// 1. Detects string array variable declarations
/// 2. Parses the comma-separated string values
/// 3. Replaces all array access patterns with literal strings
/// 4. Removes the original variable declaration
///
/// # Arguments
///
/// * `source` - The unpacked code that may contain string arrays
///
/// # Returns
///
/// The complete reconstructed code with string arrays resolved
///
/// # Examples
///
/// Input:
/// ```javascript
/// var _0x1234=["hello","world"];
/// console.log(_0x1234[0] + " " + _0x1234[1]);
/// ```
///
/// Output:
/// ```javascript
/// console.log("hello" + " " + "world");
/// ```
fn replace_strings(source: &str) -> String {
    static STRING_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"var *(_\w+)\=\["(.*?)"\];"#).unwrap());

    if let Some(caps) = STRING_REGEX.captures(source) {
        let var_name = &caps[1];
        let strings = &caps[2];
        let lookup = strings.split("\",\"").collect::<Vec<_>>();

        let mut modified_source = source.to_owned();
        for (index, value) in lookup.iter().enumerate() {
            let pattern = format!("{}[{}]", var_name, index);
            let replacement = format!("\"{}\"", value);
            modified_source = modified_source.replace(&pattern, &replacement);
        }

        let match_end = caps.get(0).unwrap().end();
        return modified_source[match_end..].to_owned();
    }

    source.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect() {
        fn positive(input: &str) {
            assert!(detect(input), "Should detect P.A.C.K.E.R. in: {}", input);
        }

        fn negative(input: &str) {
            assert!(
                !detect(input),
                "Should NOT detect P.A.C.K.E.R. in: {}",
                input
            );
        }

        negative("");
        negative("var a = b");

        positive("eval(function(p,a,c,k,e,r");
        positive("eval ( function(p, a, c, k, e, r");
    }

    #[test]
    fn test_unpack() {
        fn check(input: &str, expected: &str) {
            let result = unpack(input).unwrap();
            assert_eq!(result, expected, "Unpacking failed for input");
        }
        check(
            "eval(function(p,a,c,k,e,r){e=String;if(!''.replace(/^/,String)){while(c--)r[c]=k[c]||c;k=[function(e){return r[e]}];e=function(){return'\\\\w+'};c=1};while(c--)if(k[c])p=p.replace(new RegExp('\\\\b'+e(c)+'\\\\b','g'),k[c]);return p}('0 2=1',62,3,'var||a'.split('|'),0,{}))",
            "var a=1",
        );
        check(
            "function test (){alert ('This is a test!')}; eval(function(p,a,c,k,e,r){e=String;if(!''.replace(/^/,String)){while(c--)r[c]=k[c]||c;k=[function(e){return r[e]}];e=function(){return'\\w+'};c=1};while(c--)if(k[c])p=p.replace(new RegExp('\\b'+e(c)+'\\b','g'),k[c]);return p}('0 2=\\'{Íâ–+›ï;ã†Ù¥#\\'',3,3,'var||a'.split('|'),0,{}))",
            "function test (){alert ('This is a test!')}; var a='{Íâ–+›ï;ã†Ù¥#'",
        );
        check(
            "eval(function(p,a,c,k,e,d){e=function(c){return c.toString(36)};if(!''.replace(/^/,String)){while(c--){d[c.toString(a)]=k[c]||c.toString(a)}k=[function(e){return d[e]}];e=function(){return'\\w+'};c=1};while(c--){if(k[c]){p=p.replace(Regex('\\b'+e(c)+'\\b'),'g'),k[c])}}return p}('2 0=\"4 3!\";2 1=0.5(/b/6);a.9(\"8\").7=1;',12,12,'str|n|var|W3Schools|Visit|search|i|innerHTML|demo|getElementById|document|w3Schools'.split('|'),0,{}))",
            r#"var str="Visit W3Schools!";var n=str.search(/w3Schools/i);document.getElementById("demo").innerHTML=n;"#,
        );
        check(
            r"a=b;\r\nwhile(1){\ng=h;{return'\\w+'};break;eval(function(p,a,c,k,e,d){e=function(c){return c.toString(36)};if(!''.replace(/^/,String)){while(c--){d[c.toString(a)]=k[c]||c.toString(a)}k=[function(e){return d[e]}];e=function(){return'\\w+'};c=1};while(c--){if(k[c]){p=p.replace(new RegExp('\\b'+e(c)+'\\b','g'),k[c])}}return p}('$(5).4(3(){$('.1').0(2);$('.6').0(d);$('.7').0(b);$('.a').0(8);$('.9').0(c)});',14,14,'html|r5e57|8080|function|ready|document|r1655|rc15b|8888|r39b0|r6ae9|3128|65309|80'.split('|'),0,{}))c=abx;",
            r#"a=b;\r\nwhile(1){\ng=h;{return'\\w+'};break;$(document).ready(function(){$('.r5e57').html(8080);$('.r1655').html(80);$('.rc15b').html(3128);$('.r6ae9').html(8888);$('.r39b0').html(65309)});c=abx;"#,
        );
        check(
            "eval(function(p,a,c,k,e,r){e=function(c){return c.toString(36)};if('0'.replace(0,e)==0){while(c--)r[e(c)]=k[c];k=[function(e){return r[e]||e}];e=function(){return'[0-9ab]'};c=1};while(c--)if(k[c])p=p.replace(new RegExp('\\b'+e(c)+'\\b','g'),k[c]);return p}('$(5).a(6(){ $('.8').0(1); $('.b').0(4); $('.9').0(2); $('.7').0(3)})',[],12,'html|52136|555|65103|8088|document|function|r542c|r8ce6|rb0de|ready|rfab0'.split('|'),0,{}))",
            "$(document).ready(function(){ $('.r8ce6').html(52136); $('.rfab0').html(8088); $('.rb0de').html(555); $('.r542c').html(65103)})",
        );
    }
}
