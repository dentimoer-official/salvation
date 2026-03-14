// 주소 공간 정의
// Checker의 Polonius / borrow 검사에서 핵심으로 쓰임
//
// 속도:  thread > threadgroup > constant > device
// 안전:  thread, constant 는 안전
//        device, threadgroup 은 Checker가 엄격하게 검사

#[derive(Debug, Clone, PartialEq)]
pub enum AddressSpace {
    Device,       // GPU VRAM — read/write 가능, 가장 느림
    Constant,     // 읽기 전용 상수 버퍼 — uniform이 여기
    Threadgroup,  // 스레드 그룹 공유 — race condition 주의
    Thread,       // 스레드 로컬 — 일반 변수 기본값
}