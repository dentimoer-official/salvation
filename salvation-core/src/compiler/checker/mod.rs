// 여기서 parser에서 받은 코드가 생성 되기 전에 안전한지 볼꺼임
// 코드가 변환과 생성 직전에 문법적, 알고리즘 문제가 있나 없나 확인하는 검수 역할
// parser에서 안 하는 논리적인 문제들 다 얘가 처리함. 씹검수관

pub mod borrow_check;
pub mod data_race_check;
pub mod memory_check;
pub mod type_check;
