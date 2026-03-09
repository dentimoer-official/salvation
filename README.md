# Salvation

**Rust로 작성된 안전하고 친숙한 셰이더 언어 → Metal / Vulkan 백엔드**

Salvation은 Rust 개발자들이 **Vulkan**과 **Metal**을 어렵지 않게, 그리고 **안전하게·효율적으로** 사용할 수 있도록 설계된 **셰이더 전용 언어**입니다.

기존 셰이더 언어/프레임워크들이 가진  
- 높은 진입 장벽  
- 메모리 관리의 어려움  
- aliasing, data-race, barrier 누락 등 미묘한 버그  
같은 문제들을 최대한 줄이고, Rust스러운 안전성을 셰이더 세계로 가져오는 것을 목표로 합니다.

현재는 **Metal**(macOS/iOS) 백엔드에 집중해서 개발 중입니다.

<br>

## 현재 상태 (2026년 3월 9일 기준)

| 컴포넌트          | 상태     | 비고                                      |
|-------------------|----------|-------------------------------------------|
| Lexer             | ✓        |                                           |
| Parser            | ✓        | span 정보 포함                            |
| Type Checker      | ✓        | 불변 버퍼 쓰기 감지<br>barrier 없는 threadgroup 읽기 감지<br>미선언 변수·타입 불일치<br>struct 필드 접근<br>aliasing 체크<br>SIMD 함수 인자 검증<br>줄번호:컬럼 에러 메시지 |
| Codegen           | ✓        | MSL(Metal Shading Language) 변환<br>struct / const<br>threadgroup 지역 배열<br>SIMD 내장 함수 13종 지원 |
| CLI               | ✓        | `.slvt` → `.metal` 변환<br>`-o` 출력 옵션 지원 |

**진행 중 / 예정**
- 호스트 언어 연동 (Swift처럼 자연스럽게 부를 수 있는 방식 찾는 중)
- Data-race 방지 구조 설계
- 모바일(Metal) 지원 강화
- FFI 개선
- Zed 에디터 extension
- **Vulkan 백엔드** (2027년 11월 ~ 공식 시작 예정)

<br>

## 특별 공지

개발자(저)가 **2027년 4월**부터 약 **1년 6개월** 동안 **육군 현역**으로 입대합니다.  
→ 따라서 **Vulkan 지원은 2027년 11월경**부터 본격적으로 시작될 예정입니다.

그 전까지는 Metal 중심으로 최대한 완성도를 높이고, 안정적인 기반을 만드는 데 집중할 계획입니다.

<br>

## 철학

- Rust 개발자에게 **익숙한 문법과 소유 개념** 차용
- **unsafe** 없이도 고성능 셰이더 작성 가능
- 일반적인 GPU 프로그래밍 실수(barrier 누락, aliasing, race 등)를 **컴파일 타임**에 잡아줌
- 초보자도 비교적 쉽게 메모리 관리 가능하도록 설계

[![License](https://img.shields.io/badge/license-Unlicense-blue.svg)](http://unlicense.org/) [![Rust](https://img.shields.io/badge/Rust-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Metal](https://img.shields.io/badge/Metal-FF9900?style=flat-square&logo=apple&logoColor=white)](https://developer.apple.com/metal/)  <!-- 아직 정해지지 않았다면 -->

<br>

## 설치 & 사용 방법 (아직 초기 단계)

지금은 소스에서 직접 빌드해야 합니다.

```bash
git clone https://github.com/사용자이름/salvation.git
cd salvation
cargo run -- tests/test_1/add.slvt
