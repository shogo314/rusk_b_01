#!/usr/bin/env python3
"""Run the reference-lifetime cases under AddressSanitizer and UBSan."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CASES = [
    ('''let s:="abc";
{ let c:=s[0]; c='z'; s="XYZ"; wt c; }
s.push_back('!'); wt s;
let a := [[1,2],[3]]; let row:=a[0]; let old:=row[0];
for i in 0..1000 { a.push_back([i]); }
a=[[9]]; row[0]=8; wt old,a;
let copied = a; copied[0][0]=10; wt a,copied;
''', b'', b'X\nXYZ!\n8 9\n9 10\n'),
    ('''fn first() -> Char { let s:="abc"; return s[0]; }
let saved:Vec<Char>:=[];
for i in 0..200 { saved.push_back(first()); }
for c in saved { c='Z'; }
wt saved.len(),saved[0],saved[-1];
let detached = saved; saved=[]; wt detached.len();
let s:="a";
while s[0]=='a' { s.push_back('!'); s[0]='b'; continue; }
for i in 0..3 { let c:=s[0]; if i==1 { break; } continue; }
s.push_back('?'); wt s;
''', b'', b'200 Z Z\n200\nb!?\n'),
    ((ROOT/'examples/prototype/grid_bfs.rusk').read_text(), b'3 4\n....\n.##.\n....\n', b'5\n'),
]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--no-leaks', action='store_true', help='disable LeakSanitizer for ptrace environments')
    args=parser.parse_args()
    env=os.environ.copy()
    if args.no_leaks:
        env['ASAN_OPTIONS']=env.get('ASAN_OPTIONS','')+':detect_leaks=0'
    with tempfile.TemporaryDirectory(prefix='rusk-sanitizers-') as directory:
        temp=Path(directory)
        for source, data, expected in CASES:
            (temp/'main.rusk').write_text(source)
            subprocess.run([str(ROOT/'target/debug/rusk'),'compile',str(temp/'main.rusk'),'-o',str(temp/'main.cpp')],check=True)
            subprocess.run(['g++','-std=c++20','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',str(temp/'main.cpp'),'-o',str(temp/'main')],check=True)
            run=subprocess.run([str(temp/'main')],input=data,capture_output=True,env=env)
            assert run.returncode == 0, run.stderr.decode(errors="replace")
            assert run.stdout==expected, (run.stdout,expected)
            assert not run.stderr, run.stderr
    print(f'AddressSanitizer / UBSan: {len(CASES)} cases passed')

if __name__ == '__main__':
    main()
