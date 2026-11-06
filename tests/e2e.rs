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

#[test]
fn arrays_store_and_rebind_shared_elements() {
    succeeds(
        r#"
let x:=1;
let a:Vec<Int> := [x, 2];
let held:=a[0];
x=3; wt a;
a[0]=4; wt x,held,a;
let y:=10;
(a[0]):=y;
held=5; y=11;
wt a,x,held;
a[-1]+=1; wt a[-2],a[-1];
let b:=a;
push_back(b,x); x=6;
wt len(a),a;
let result: Int;
result := (a[0] := y := 12);
result=13;
wt a,y;
"#,
        "",
        "3 2\n4 4 4 2\n11 2 5 5\n11 3\n3 11 3 6\n13 3 6 13\n",
    );
}

#[test]
fn arrays_copy_values_and_preserve_only_outer_identity_on_assignment() {
    succeeds(
        r#"
let x:=1;
let a:Vec<Int> := [x,2];
let alias:=a;
let old_element:=a[0];
let independent:=copy(a);
a[0]=7; wt a, independent;
let source:Vec<Int> := [10,20];
a=source;
source[0]=99;
wt alias,old_element,source;
a[0]=11;
wt independent;
let before:=a[0];
a=a;
a[0]=12;
wt a,before;
a:=copy(source);
wt a,alias;
"#,
        "",
        "7 2 1 2\n10 20 7 99 20\n1 2\n12 20 11\n99 20 12 20\n",
    );
}

#[test]
fn array_element_lifetime_survives_reallocation_and_scope() {
    succeeds(
        r#"
let a:Vec<Int> := [1];
let held:=a[0];
let i:=0;
while i<200 { push_back(a, i+10); i+=1; }
held=5; wt a[0],len(a);
let escaped: Int;
{ let temporary:Vec<Int> := [9]; escaped:=temporary[0]; }
escaped+=1; wt escaped;
a=[]; held=6; wt len(a),held;
if a { wt "wrong"; } else { wt "empty"; }
if [1] { wt "nonempty"; }
"#,
        "",
        "5 201\n10\n0 6\nempty\nnonempty\n",
    );
}

#[test]
fn array_index_targets_are_evaluated_once_before_rhs() {
    succeeds(
        r#"
let a:Vec<Int> := [10,20,30];
let i:=0;
a[(i+=1)]+= (i+=1);
wt a,i;
a[0] = (a[0] := 99);
wt a;
let x:=1;
let literal := [x, (x:=2)];
wt literal;
let b:Vec<Int> := [3];
a[0] := (b[0] := 4);
b[0]=5; wt a,b;
"#,
        "",
        "10 22 30 2\n99 22 30\n1 2\n5 22 30 5\n",
    );
}

#[test]
fn invalid_arrays_are_rejected_by_rusk() {
    for source in [
        "let a := [];",
        "let a:Vec<Bool> := [true];",
        "let a := [1,true];",
        "let a := [[1]];",
        "let a := [1]; wt a[true];",
        "let a := [1]; a[0]:=false;",
        "let a := [1]; push_back(a,false);",
        "let a := [1]; wt a[0][0];",
    ] {
        let out = Case::new(source).compile();
        assert!(!out.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:"));
    }
    for source in [
        "let a:Vec<Int>:=[]; wt a[0];",
        "let a:Vec<Int>:=[1]; wt a[-2];",
        "let a:Vec<Int>:=[1]; a[1]:=2;",
        "let a:Vec<Int>:=[1]; a[1]=2;",
        "let a:Vec<Int>:=[1]; wt a[-9223372036854775808];",
        "let a:Vec<Int>:=[1]; wt a[9223372036854775807];",
    ] {
        let out = Case::new(source).run("");
        assert!(!out.status.success(), "{source}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains("main.rusk:1:"), "{error}");
        assert!(error.contains("array index out of range"), "{error}");
    }
}

#[test]
fn reverse_array_example() {
    succeeds(
        include_str!("../examples/array.rusk"),
        "5 1 2 3 4 5",
        "5 4 3 2 1\n",
    );
    succeeds(include_str!("../examples/array.rusk"), "0", "\n");
}

#[test]
fn empty_arrays_use_parameter_context_and_concrete_external_types() {
    succeeds(
        r#"
extern fn first(a: Vec<Int>) -> Int {
    cpp_name: "first_vec_int",
    code: "Obj<std::int64_t> first_vec_int(Obj<IntVec> a, Loc loc) { return (*a)[vec_int_index(*a, 0, loc)]; }",
}
let a: Vec<Int> := [];
push_back(a,1);
first(a)=2;
wt len([]),a;
__assign__(a,[]);
wt len(a);
"#,
        "",
        "0 2\n0\n",
    );
}

#[test]
fn repeated_arrays_copy_each_element_independently() {
    succeeds(
        r#"
let x:=7;
let a := [x;3];
a[0]=10;
x=8;
wt a,x;
let n:=0;
let empty := [(n+=1);0];
wt empty.len(),n;
let repeated := [(n+=1);(n+=1)];
wt repeated,n;
n+=1;
wt repeated,n;
"#,
        "",
        "10 7 7 8\n0 1\n3 3 3 3\n3 3 3 4\n",
    );
}

#[test]
fn method_calls_desugar_to_functions_and_preserve_evaluation_order() {
    succeeds(
        r#"
let a := [1,2];
a.push_back(3);
wt a.len(), a.copy()[1], a[0].copy();
let independent:=a.copy();
independent[0]=9;
wt a,independent;
let old:=a;
a.push_back((a:=[8])[0]);
wt old,a;
let value:=1;
wt value.__add__(2), value;
[0;0].len().__write__(); wt;
"#,
        "",
        "3 2 1\n1 2 3 9 2 3\n1 2 3 8 8\n3 1\n0\n",
    );
}

#[test]
fn invalid_repetition_and_method_calls_are_located() {
    for source in [
        "let a := [true;3];",
        "let a := [1;false];",
        "[1].missing();",
        "[1].len(2);",
        "let a := [1;2,3];",
    ] {
        let out = Case::new(source).compile();
        assert!(!out.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk"));
    }
    for (source, message) in [
        ("let a := [0;-1];", "negative array length"),
        (
            "let a := [0;9223372036854775807];",
            "array length out of range",
        ),
    ] {
        let out = Case::new(source).run("");
        assert!(!out.status.success(), "{source}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains("main.rusk:1:"), "{error}");
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn frequency_example_uses_independent_counters() {
    succeeds(
        include_str!("../examples/frequency.rusk"),
        "6 4\n0 1 0 3 1 0\n",
        "3 2 0 1\n",
    );
}

#[test]
fn rusk_functions_support_recursion_forward_calls_and_overloads() {
    succeeds(
        r#"
wt factorial(6), even(10), even(11);
fn factorial(n: Int) -> Int {
    if n<=1 { return 1; }
    return n*factorial(n-1);
}
fn even(n: Int) -> Bool {
    if n==0 { return true; } else { return odd(n-1); }
}
fn odd(n: Int) -> Bool {
    if n==0 { return false; } else { return even(n-1); }
}
fn measure(x: Int) -> Int { return x+1; }
fn measure(x: Vec<Int>) -> Int { return x.len(); }
wt measure(3), [1,2].measure();
"#,
        "",
        "720 Yes No\n4 2\n",
    );
    succeeds(include_str!("../examples/gcd.rusk"), "48 18", "6\n");
}

#[test]
fn function_arguments_and_returns_preserve_sharing() {
    succeeds(
        r#"
fn update(x: Int) { x=5; x:=99; }
fn first(a: Vec<Int>) -> Int { return a[0]; }
fn make() -> Vec<Int> { let a := [1,2]; return a; }
fn reset(a: Vec<Int>) { a=[7,8]; a:=[9]; }
let x:=1; update(x); wt x;
let a:=make(); let held:=a[0];
first(a)=4; wt a,held;
reset(a); wt a,held;
let saved:=make()[0]; saved+=1; wt saved;
"#,
        "",
        "5\n4 2 4\n7 8 4\n2\n",
    );
}

#[test]
fn function_returns_handle_empty_arrays_unit_and_early_exit() {
    succeeds(
        r#"
fn empty() -> Vec<Int> { return []; }
fn show(x: Int) {
    if x<0 { wt "negative"; return; }
    wt x;
}
fn done() -> Unit { return; }
fn delegate() { return done(); }
let a:=empty(); wt a.len();
show(-1); show(3); delegate();
"#,
        "",
        "0\nnegative\n3\n",
    );
}

#[test]
fn user_functions_extend_operators_with_existing_resolution() {
    succeeds(
        r#"
fn __iadd__(x: Int, y: Int) { x = x+y+10; }
let a:=1; a+=2; wt a;
"#,
        "",
        "13\n",
    );
}

#[test]
fn invalid_function_definitions_and_returns_are_located() {
    for source in [
        "return;",
        "return 1;",
        "fn f() -> Int {}",
        "fn f(x:Bool) -> Int { if x { return 1; } }",
        "fn f() -> Int { return; }",
        "fn f() { return 1; }",
        "fn f() -> Int { return true; }",
        "fn f(x:Int,x:Int) {}",
        "fn f(x:Int) {} fn f(x:Int) {}",
        "fn f() -> Int { return 1; } fn f() -> Bool { return true; }",
        "fn f(x:Missing) {}",
        "fn f() -> Missing { return 1; }",
        "fn outer() { fn inner() {} }",
        "{ fn nested() {} }",
        "let global:=1; fn f() -> Int { return global; }",
        "fn unused() { wt missing; }",
        "fn f() { break; }",
        "fn f() { continue; }",
        "fn f(x:Int) { let x:=1; }",
        "fn f() { extern type Local { cpp_name: \"int\", } }",
        "fn f() { let x:Int; wt x; }",
    ] {
        let case = Case::new(source);
        let out = case.compile();
        assert!(!out.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("main.rusk:"),
            "{source}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!case.0.join("main.cpp").exists());
    }
}

#[test]
fn runtime_errors_in_functions_report_the_operation_position() {
    let out = Case::new("fn divide(x: Int) -> Int {\n return 1/x;\n}\nwt divide(0);").run("");
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(error.contains("main.rusk:2:"), "{error}");
}

#[test]
fn initialization_merges_only_function_branches_that_continue() {
    succeeds(
        r#"
fn choose(flag: Bool) -> Int {
    let x: Int;
    if flag { return 1; } else { x:=2; }
    return x;
}
fn choose_other(flag: Bool) -> Int {
    let x: Int;
    if flag { x:=3; } else { return 4; }
    return x;
}
wt choose(true),choose(false),choose_other(true),choose_other(false);
"#,
        "",
        "1 2 3 4\n",
    );
}
