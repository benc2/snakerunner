use std::str::FromStr;

use crate::showgame::parse_player_move;
use crate::Direction;

#[derive(Debug, PartialEq)]
pub enum Instruction {
    AskMove,
    Move { player: usize, direction: Direction },
    Out { player: usize },
    Stop,
}

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("Could not be parsed to instruction")]
pub struct InstructionParseError {}

impl FromStr for Instruction {
    type Err = InstructionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Instruction::*;
        match s {
            "stop" => Ok(Stop),
            "move" => Ok(AskMove),
            instr if &instr[..3] == "out" => {
                let player = *&instr[4..]
                    .parse::<usize>()
                    .map_err(|_| InstructionParseError {})?;
                Ok(Out { player })
            }
            instr => {
                let (player, direction) =
                    parse_player_move(instr).map_err(|_| InstructionParseError {})?;
                Ok(Move { player, direction })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::*;
    use Instruction::*;
    #[test]
    fn stop() {
        assert_eq!("stop".parse(), Ok(Stop));
    }

    #[test]
    fn ask_move() {
        assert_eq!("move".parse(), Ok(AskMove));
    }

    #[test]
    fn out() {
        assert_eq!("out:12".parse(), Ok(Out { player: 12 }));
        assert_eq!(
            "out:-1".parse::<Instruction>(),
            Err(InstructionParseError {})
        );
        assert_eq!(
            "out:bloop".parse::<Instruction>(),
            Err(InstructionParseError {})
        );
    }

    #[test]
    fn move_test() {
        assert_eq!(
            "12:N".parse(),
            Ok(Move {
                player: 12,
                direction: North
            })
        );
        assert_eq!(
            "13:S\n".parse(),
            Ok(Move {
                player: 13,
                direction: South
            })
        );
        assert_eq!(
            "14:zoom".parse::<Instruction>(),
            Err(InstructionParseError {})
        );
    }
}
