
use pest::Parser;
use std::fmt;

#[derive(pest_derive::Parser)]
#[grammar = "command_grammer.pest"]
pub struct Command;

type PestError = pest::error::Error<Rule>;

#[derive(Debug)]
pub enum ParseError {
    Pest(PestError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pest(err) => err.fmt(f),
        }
    }
}

impl From<PestError> for ParseError {
    fn from(err: PestError) -> Self {
        Self::Pest(err)
    }
}

pub fn parse_command(command: &str) -> Result<(&str, Vec<&str>), ParseError> {
    let content = command;
    match Command::parse(Rule::command, content.trim()) {
        Ok(mut pairs) => {
            // let thing: Rule = pairs.next().unwrap();
            // println!("{:?}", pairs);
            // dbg!(&pairs);
            let command = pairs.next().unwrap();
            let mut command_pairs = command.into_inner();
            // dbg!(&pairs);
            // assert_eq!(command_pairs.next().map(|x| x.as_rule()), Some(Rule::SOI));
            let command_name = command_pairs.next().unwrap().as_str();
            let args: Vec<_> = command_pairs
                .by_ref()
                .take_while(|pair| {
                    pair.as_rule() == Rule::argument
                })
                .map(|pair| {
                    // let pair = pairs.next().unwrap();
                    // let pair: u32 = pair;
                    let raw_arg = pair.into_inner().next().unwrap();
                    // dbg!(&raw_arg);
                    match raw_arg.as_rule() {
                        Rule::argument_quoted => {
                            raw_arg.into_inner().next().unwrap().as_str()
                        }
                        Rule::argument_unquoted => {
                            raw_arg.as_str()
                        }
                        _ => panic!("Unreachable arg type {:?}", raw_arg),
                    }
                    
                    // pair.as_str()
                })
                .collect();
            // assert_eq!(command_pairs.next().map(|x| x.as_rule()), Some(Rule::EOI));
            // dbg!(&command_name, &args);
            // reply!(format!("{} {:?}", command_name, args));
            Ok((command_name, args))
        }
        Err(err) => {
            // dbg!(&err);
            // let loc = err.location.0;
            // println!("{}", err.line);
            // for _ in 0..loc {
            //     print!(" ");
            // }
            // println!("^");
            // println!("{}", err);
            // panic!()
            Err(ParseError::Pest(err))
            // ("", Vec::new())
        }
    }
}
