use crate::cursor::Cursor;
use crate::error::{ColdTrailError, Result};
use crate::model::{ScriptInstruction, ScriptProgram};

pub fn parse_script(input: &[u8]) -> Result<ScriptProgram> {
    let mut cursor = Cursor::new(input);
    let mut program = ScriptProgram::default();
    let mut budget = 2048usize;
    while !cursor.is_empty() && program.instructions.len() < 512 {
        budget = budget.checked_sub(1).ok_or(ColdTrailError::ScriptBudget)?;
        let opcode = cursor.read_u8()?;
        match opcode {
            0x00 => program.instructions.push(ScriptInstruction::Stop),
            0x01 => {
                let value = cursor.read_i32_le()?;
                program.instructions.push(ScriptInstruction::LoadConst(value));
            }
            0x02 => {
                let code = cursor.read_u16_le()?;
                program.instructions.push(ScriptInstruction::LoadSensor(code));
            }
            0x03 => {
                let slot = cursor.read_u8()?;
                program.instructions.push(ScriptInstruction::StoreSlot(slot));
            }
            0x04 => program.instructions.push(ScriptInstruction::Add),
            0x05 => program.instructions.push(ScriptInstruction::Sub),
            0x06 => program.instructions.push(ScriptInstruction::Mul),
            0x07 => {
                let min = cursor.read_i32_le()?;
                let max = cursor.read_i32_le()?;
                program.instructions.push(ScriptInstruction::Clamp { min, max });
            }
            0x08 => {
                let channel = cursor.read_u8()?;
                program.emits += 1;
                program.instructions.push(ScriptInstruction::Emit { channel });
            }
            0x09 => {
                let label = cursor.read_u8()?;
                program.labels.push(label);
            }
            0x0a => {
                let slot = cursor.read_u8()?;
                let threshold = cursor.read_i32_le()?;
                let target = cursor.read_u8()?;
                program
                    .instructions
                    .push(ScriptInstruction::JumpIfBelow { slot, threshold, target });
            }
            0x0b => {
                let station = cursor.read_u16_le()?;
                program.instructions.push(ScriptInstruction::MarkStation(station));
            }
            0x80..=0x8f => {
                let count = usize::from(opcode & 0x0f);
                for _ in 0..count {
                    if cursor.remaining() < 4 {
                        break;
                    }
                    let value = cursor.read_i32_le()?;
                    program.instructions.push(ScriptInstruction::LoadConst(value));
                }
            }
            other => return Err(ColdTrailError::ScriptOpcode(other)),
        }
        if matches!(program.instructions.last(), Some(ScriptInstruction::Stop)) {
            break;
        }
    }
    analyze_program(&program)?;
    Ok(program)
}

fn analyze_program(program: &ScriptProgram) -> Result<()> {
    let mut depth = 0i32;
    for ins in &program.instructions {
        match ins {
            ScriptInstruction::LoadConst(_) | ScriptInstruction::LoadSensor(_) => depth += 1,
            ScriptInstruction::StoreSlot(_) | ScriptInstruction::Emit { .. } => depth -= 1,
            ScriptInstruction::Add | ScriptInstruction::Sub | ScriptInstruction::Mul => depth -= 1,
            ScriptInstruction::Clamp { .. } => {}
            ScriptInstruction::JumpIfBelow { .. } => {}
            ScriptInstruction::MarkStation(_) => {}
            ScriptInstruction::Stop => break,
        }
        if depth < -8 || depth > 64 {
            return Err(ColdTrailError::ScriptStack);
        }
    }
    Ok(())
}
