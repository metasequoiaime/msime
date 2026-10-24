from pathlib import Path
import argparse,json,subprocess
parser=argparse.ArgumentParser()
parser.add_argument("--variant",choices=["utf8","bytes"],required=True)
args=parser.parse_args()
root=Path.cwd(); folder=root/'target/next-hotspot-evidence'/args.variant
folder.mkdir(parents=True,exist_ok=True)
source=r'''
use std::hint::black_box;
use std::time::Instant;
#[path="__CACHE__"] mod cache;
mod types { #[derive(Clone)] pub struct WordItem(pub u64); }
#[path="__PATH_CACHE__"] mod path_cache;
use cache::FifoCache;
use path_cache::PathCache;
fn now() -> u64 {
    let mut time = libc::timespec {tv_sec:0,tv_nsec:0};
    assert_eq!(unsafe {libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID,&mut time)},0);
    time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64
}
fn run(new:bool, keys:&[String], rows:usize, mode:usize, iterations:usize) -> u64 {
    let capacity=if keys.len()>128 {128} else {1};
    let mut old=FifoCache::new(capacity);
    let mut candidate=PathCache::new(capacity);
    let page=vec![types::WordItem(17); rows];
    for key in keys.iter().take(capacity) {
        if new {candidate.insert_ref(key,page.clone());} else {old.insert(key.to_owned(),page.clone());}
    }
    let start=now();
    for index in 0..iterations {
        let key=black_box(&keys[if mode==2 {0} else if mode==1 {index%capacity} else {index%keys.len()}]);
        if mode==1 {
            if new {black_box(candidate.get_ref_by(key));} else {black_box(old.get_ref_by(key.as_str()));}
        } else if new {candidate.insert_ref(key,black_box(page.clone()));}
        else {old.insert(key.to_owned(),black_box(page.clone()));}
    }
    black_box((old,candidate));
    now()-start
}
fn main(){
    let wall=Instant::now();
    for length in [0usize,1,8,23,31,32,64] {
        let keys:Vec<String>=if length==0 {vec![String::new()]} else if length==1 {vec!["a".into(),"b".into()]} else {(0..256).map(|n|format!("{n:03}{}","x".repeat(length-3))).collect()};
        for rows in [0usize,1,32] {for mode in 0..3 {
            let mut ratios=Vec::new();
            for pair in 0..21 {
                let (a,b)=if pair%2==0 {(run(false,&keys,rows,mode,20000),run(true,&keys,rows,mode,20000))} else {let b=run(true,&keys,rows,mode,20000);(run(false,&keys,rows,mode,20000),b)};
                ratios.push(b as f64/a as f64);
            }
            ratios.sort_by(f64::total_cmp);
            println!("{{\"length\":{length},\"rows\":{rows},\"mode\":{mode},\"ratio\":{},\"ratios\":{:?}}}",ratios[ratios.len()/2],ratios);
        }}
    }
    eprintln!("wall_seconds={}",wall.elapsed().as_secs_f64());
}
'''.replace('__CACHE__',str(root/'crates/engine/src/cache.rs')).replace('__PATH_CACHE__',str(Path(__file__).resolve().parent/f'{args.variant}-path_cache.rs'))
(folder/'bench.rs').write_text(source)
extern=[]
for name in ['lru','libc']:
    paths=list((root/'target/release/deps').glob(f'lib{name}-*.rlib'))
    assert len(paths)==1,(name,paths)
    extern += ['--extern',f'{name}={paths[0]}']
subprocess.run(['rustc','--edition=2021','-O','-A','dead_code',str(folder/'bench.rs'),'-L',f'dependency={root / "target/release/deps"}',*extern,'-o',str(folder/'bench')],check=True)
with (folder/'bench.jsonl').open('w') as out, (folder/'bench-run.log').open('w') as err:
    subprocess.run([str(folder/'bench')],stdout=out,stderr=err,check=True)
records=[json.loads(line) for line in (folder/'bench.jsonl').read_text().splitlines()]
summary={'cells':len(records),'targets':[r for r in records if 0<r['length']<=31 and r['mode']!=1],'regressed_controls':[r for r in records if (r['length']==0 or r['length']>31 or r['mode']==1) and r['ratio']>1.05]}
(folder/'bench-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({'cells':summary['cells'],'target_ratios':[r['ratio'] for r in summary['targets']],'regressed_controls':[{k:v for k,v in r.items() if k!='ratios'} for r in summary['regressed_controls']]},indent=2))
