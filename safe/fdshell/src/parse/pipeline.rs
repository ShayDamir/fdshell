use crate::parse::command::parse_command;
use crate::parse::{ParsedLine, Pipeline};
use sys::ShortCStr;
use sys::errno::EINVAL;

pub fn parse_pipeline(raw: &[ShortCStr]) -> Result<ParsedLine, i32> {
    let mut commands = Vec::new();
    let mut start = 0;
    for (i, t) in raw.iter().enumerate() {
        if t.as_bytes()? == b"|" {
            if i == start {
                return Err(EINVAL);
            }
            commands.push(parse_command(raw.get(start..i).ok_or(EINVAL)?)?);
            start = i + 1;
        }
    }
    if start >= raw.len() {
        return Err(EINVAL);
    }
    commands.push(parse_command(raw.get(start..).ok_or(EINVAL)?)?);
    Ok(ParsedLine::Pipeline(Pipeline { commands }))
}
