use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Case(PathBuf);
impl Case {
    fn new(source: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "rusk-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.rusk"), source).unwrap();
        Self(dir)
    }
    fn compile(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rusk"))
            .arg("compile")
            .arg(self.0.join("main.rusk"))
            .arg("-o")
            .arg(self.0.join("main.cpp"))
            .output()
            .unwrap()
    }
    fn run(&self, input: &str) -> Output {
        let compiled = self.compile();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let native = Command::new("g++")
            .args(["-std=c++20", "-O2", "-Wall", "-Wextra"])
            .arg(self.0.join("main.cpp"))
            .arg("-o")
            .arg(self.0.join("main"))
            .output()
            .unwrap();
        assert!(
            native.status.success(),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        let mut child = Command::new(self.0.join("main"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn succeeds(source: &str, input: &str, expected: &str) {
    let out = Case::new(source).run(input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}
#[test]
fn input_and_output() {
    succeeds(
        "in a, b: Int; in s: Str; in c: Char; wt a+b, s, c, true, false; wt;",
        "10 +32 hello Z",
        "42 hello Z Yes No\n\n",
    );
    succeeds(r#"wt "a\u0000b", '\n';"#, "", "a\0b \n\n");
    succeeds(
        "in n: Int; wt read_line(); wt read_line();",
        "12 remainder\nnext\n",
        " remainder\nnext\n",
    );
}
#[test]
fn sharing_copy_rebinding_and_scope() {
    succeeds(
        "let a := 10; let b := a; let c := copy(a); a += 1; wt a,b,c; a := 20; wt a,b; a = 30; wt a,b; { let a := b; a = 7; } wt a,b; let z: Int; z := 5; wt z;",
        "",
        "11 11 10\n20 11\n30 11\n30 7\n5\n",
    );
}
#[test]
fn precedence_and_assignment_chains() {
    succeeds(
        "let a:=1; let b:=2; a = b = 3; wt a,b, 2+3*4, -(2+3); a := b := 8; b += 1; wt a,b;",
        "",
        "3 3 14 -5\n9 9\n",
    );
}
#[test]
fn evaluation_order_preserves_objects() {
    succeeds(
        r#"
extern fn pair(a: Int, b: Int) -> Int {
 cpp_name: "pair_values",
 code: "Obj<std::int64_t> pair_values(Obj<std::int64_t> a, Obj<std::int64_t> b, Loc) { return make_obj<std::int64_t>(*a * 10 + *b); }",
}
let a:=1; wt pair(a, a = 2); wt pair(a, a := 3); wt a;
"#,
        "",
        "22\n23\n3\n",
    );
}
#[test]
fn integer_floor_division_and_boundaries() {
    succeeds(
        "wt -7/3,-7%3,7/-3,7%-3,-7/-3,-7%-3; wt -9223372036854775808,9223372036854775807; wt -9223372036854775808%-1;",
        "",
        "-3 2 -3 -2 2 -1\n-9223372036854775808 9223372036854775807\n0\n",
    );
}
#[test]
fn runtime_errors_have_source_locations() {
    for expression in [
        "9223372036854775807+1",
        "-9223372036854775808-1",
        "9223372036854775807*2",
        "-(-9223372036854775808)",
        "-9223372036854775808/-1",
        "1/0",
        "1%0",
    ] {
        let out = Case::new(&format!("wt {expression};")).run("");
        assert!(!out.status.success(), "{expression}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:1:"));
    }
}
#[test]
fn invalid_input_is_rejected() {
    let case = Case::new("in n: Int; wt n;");
    // Compile once, exercise the same generated executable with multiple inputs.
    assert!(case.run("-9223372036854775808").status.success());
    for input in ["", "x", "12x", "+-1", "9223372036854775808"] {
        let mut child = Command::new(case.0.join("main"))
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(!out.status.success(), "{input:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:1:"));
    }
}
#[test]
fn compile_errors_are_located_and_do_not_write_output() {
    for source in [
        "wt missing;",
        "let a: Int; wt a;",
        "let a:=1; let a:=2;",
        "let a:=1; a:=true;",
        "let a: Missing;",
        "let a;",
        "wt 9223372036854775808;",
        "{ extern type X { cpp_name: \"int\", } }",
        "wt ; @",
    ] {
        let case = Case::new(source);
        let out = case.compile();
        assert!(!out.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk"));
        assert!(!case.0.join("main.cpp").exists());
    }
}
#[test]
fn native_inplace_operator_is_preferred() {
    succeeds(
        r#"
extern fn __iadd__(a: Int, b: Int) {
 cpp_name: "custom_iadd",
 code: "void custom_iadd(Obj<std::int64_t> a, Obj<std::int64_t> b, Loc) { *a += *b + 100; }",
}
let a:=1; let b:=a; a+=2; wt a,b;
"#,
        "",
        "103 103\n",
    );
}

#[test]
fn custom_type_and_operator_are_defined_in_rusk() {
    succeeds(
        r#"
extern type Score { cpp_name: "double", }
extern fn score(x: Int) -> Score {
 cpp_name: "score_from_int",
 code: "Obj<double> score_from_int(Obj<std::int64_t> x, Loc) { return make_obj<double>(*x); }",
}
extern fn __add__(a: Score, b: Score) -> Score {
 cpp_name: "add_score",
 code: "Obj<double> add_score(Obj<double> a, Obj<double> b, Loc) { return make_obj<double>(*a + *b); }",
}
extern fn __write__(x: Score) {
 cpp_name: "write_score",
 code: "void write_score(Obj<double> x, Loc) { std::cout << *x; }",
}
wt score(2) + score(3);
"#,
        "",
        "5\n",
    );
}

#[test]
fn loops_can_read_and_sum_a_sequence() {
    succeeds(
        include_str!("../examples/sum.rusk"),
        "5\n1 -2 3 4 5\n",
        "11\n",
    );
    succeeds(
        "let a:=48; let b:=18; while b { let r:=a%b; a:=b; b:=r; } wt a;",
        "",
        "6\n",
    );
}

#[test]
fn conditionals_and_comparisons() {
    succeeds(
        r#"
in n: Int;
let result: Str;
if n < 0 { result := "negative"; } else { result := "nonnegative"; }
wt result;
wt 1==1, 1!=1, 1<=2, 2>1, 2>=2, "a"<"b", 'a'<'b', true==false;
if "" { wt "wrong"; } else { wt "empty"; }
if "hello" { wt "nonempty"; }
if false { wt "wrong"; }
if -1 { wt "nonzero"; }
"#,
        "-1",
        "negative\nYes No Yes Yes Yes Yes Yes No\nempty\nnonempty\nnonzero\n",
    );
}

#[test]
fn break_continue_and_loop_else() {
    succeeds(
        r#"
let i:=0;
let total:=0;
while i<5 {
    i+=1;
    if i==2 { continue; }
    total+=i;
} else { wt "finished",total; }
while true { break; } else { wt "wrong"; }
while false { wt "wrong"; } else { wt "zero"; }
loop { total+=1; if total==15 { break; } }
wt total;
let outer:=0;
while outer<2 {
    outer+=1;
    while true { break; } else { wt "wrong"; }
} else { wt "outer finished"; }
"#,
        "",
        "finished 13\nzero\n15\nouter finished\n",
    );
}

#[test]
fn control_flow_checks_definite_initialization() {
    for source in [
        "let x:Int; if true { x:=1; } wt x;",
        "let x:Int; if true { x:=1; } else { wt x; }",
        "let x:Int; while false { x:=1; } wt x;",
        "let x:Int; loop { x:=1; break; } wt x;",
        "let x:Int; while false {} else { x:=1; } wt x;",
        "break;",
        "continue;",
        "if true { let x:=1; } wt x;",
    ] {
        let case = Case::new(source);
        let out = case.compile();
        assert!(!out.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:"));
    }
    succeeds("let x:Int; while (x:=0) {} wt x;", "", "0\n");
}
