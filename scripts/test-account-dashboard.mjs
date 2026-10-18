import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import test from "node:test";
import ts from "typescript";
const source = ts.transpileModule(readFileSync(new URL("../src/utils/accountDashboard.ts", import.meta.url), "utf8"), {compilerOptions:{module:ts.ModuleKind.ES2022,target:ts.ScriptTarget.ES2022}}).outputText;
const {dashboardPools,dashboardAccountState,dashboardCounts,quotaPoolKind} = await import(`data:text/javascript;base64,${Buffer.from(source).toString("base64")}`);
const now = Date.parse("2026-10-02T12:00:00Z");
const bucket = (id,remaining=0.8) => ({bucket_id:id,window:id,remaining_fraction:remaining,reset_time:"2026-10-03T12:00:00Z"});
const quota = () => ({last_updated:now/1000-20,is_forbidden:false,subscription_tier:"PRO",provenance:"observed",models:[],groups:[{key:'buckets:["5h","weekly"]',display_name:"Shared",buckets:[bucket("5h"),bucket("weekly",0.5)]}]});
const account = () => ({id:"fixture",email:"fixture@example.test",name:null,custom_label:null,read_status:"loaded",read_error:null,disabled:false,validation_blocked:false,validation_blocked_until:null,protected_models:[],quota:quota()});
test('canonical quota pools agree with Rust and keep future model pools separate', () => {
  const cases = JSON.parse(readFileSync(new URL('../tests/fixtures/quota-pool-cases.json', import.meta.url), 'utf8'));
  for (const c of cases) assert.equal(quotaPoolKind(c.id, c.name), c.pool);
});
test('a reported Argon weekly pool is complete without an invented five-hour window', () => {
  const a = account();
  a.quota.groups.push({display_name:'Gemini 4 Argon',buckets:[{bucket_id:'gemini-argon-wtu-weekly',window:'weekly',remaining_fraction:0.1,reset_time:'2026-10-03T12:00:00Z'}]});
  assert.equal(dashboardAccountState(a,now).quota,'reported');
  assert.equal(dashboardPools(a.quota).at(-1).windows[0].remaining,10);
});
test("null is unknown; explicitly reported zero is exhausted",()=>{
  const a=account(); a.quota.groups[0].buckets[0].remaining_fraction=null;
  assert.equal(dashboardPools(a.quota)[0].windows[0].remaining,null);
  assert.equal(dashboardAccountState(a,now).quota,"partial");
  a.quota.groups[0].buckets[0].remaining_fraction=0;
  assert.equal(dashboardPools(a.quota)[0].windows[0].remaining,0);
  assert.equal(dashboardAccountState(a,now).quota,"exhausted");
});
test("models sharing a pool are labels and do not create extra quota rows",()=>{
  const q=quota(); q.models=["model-a","model-b"].map(name=>({name,percentage:80,display_name:null,reset_time:"",inferred_bucket_id:"5h"}));
  const pools=dashboardPools(q);
  assert.equal(pools.length,1); assert.equal(pools[0].windows.length,2);
  assert.deepEqual(pools[0].models,["model-a","model-b"]);
  assert.equal(pools[0].mapping,"inferred");
  assert.deepEqual(pools[0].windows.map(w=>w.remaining),[80,50]);
});
test("duplicate group windows count once; conflicts become unknown",()=>{
  const q=quota(); q.groups.push(structuredClone(q.groups[0]));
  assert.equal(dashboardPools(q).length,1); assert.equal(dashboardPools(q)[0].windows.length,2);
  q.groups[1].buckets[0].remaining_fraction=0.2;
  assert.equal(dashboardPools(q)[0].windows[0].remaining,null);
});
test("missing groups and unresolved mapping are not silently discarded",()=>{
  const a=account(); a.quota.groups.push({key:null,display_name:"Missing group",buckets:[]});
  assert.equal(dashboardAccountState(a,now).quota,"partial");
  a.quota.groups.pop(); a.quota.models.push({name:"unmapped",percentage:70,display_name:null,reset_time:"",inferred_bucket_id:null});
  assert.equal(dashboardPools(a.quota).length,2);
  assert.equal(dashboardAccountState(a,now).quota,"partial");
  a.quota.models[0].inferred_bucket_id="missing-bucket";
  assert.equal(dashboardPools(a.quota)[1].windows[0].remaining,null);
});
test("one missing window is partial even without another account to compare",()=>{
  const a=account(); a.quota.groups[0].buckets.pop();
  assert.equal(dashboardAccountState(a,now).quota,"partial");
  a.quota.groups[0].buckets[0].window="unrecognized-window";
  assert.equal(dashboardAccountState(a,now).quota,"partial");
});
test("authentication, quota and freshness remain independent observations",()=>{
  const a=account(); a.disabled=true; a.quota.last_updated-=3600;
  const state=dashboardAccountState(a,now);
  assert.equal(state.authentication,"disabled"); assert.equal(state.quota,"reported"); assert.equal(state.freshness,"stale");
  a.disabled=false; a.quota.is_forbidden=true;
  assert.equal(dashboardAccountState(a,now).authentication,"not_verified");
  assert.equal(dashboardAccountState(a,now).quota,"forbidden");
  a.validation_blocked=true; assert.equal(dashboardAccountState(a,now).authentication,"verification_required");
  a.validation_blocked_until=now/1000-1; assert.equal(dashboardAccountState(a,now).authentication,"not_verified");
});
test("legacy provenance and expired reset details are retained",()=>{
  const a=account(); a.quota.provenance="legacy_cache"; a.quota.groups[0].buckets[0].remaining_fraction=null;
  a.quota.groups[0].buckets[0].reset_time="2026-10-01T12:00:00Z";
  const state=dashboardAccountState(a,now);
  assert.equal(state.provenance,"legacy_cache"); assert.equal(state.resetExpired,true); assert.equal(state.quota,"partial");
});
test("totals come from the index/read contract and current is only a Tools record",()=>{
  const a=account(); a.read_status="failed"; a.quota=null;
  const result=dashboardCounts({indexed_total:10,loaded_count:8,failed_count:2,current_account_id:"unreadable",current_identity_source:"tools_record",accounts:[a]});
  assert.deepEqual(result,{indexed:10,loaded:8,failed:2,currentAccountId:"unreadable",currentIdentitySource:"tools_record"});
  assert.equal(dashboardAccountState(a,now).authentication,"unknown");
  assert.equal(dashboardAccountState(a,now).quota,"unknown");
  assert.equal(dashboardAccountState(a,now).freshness,"missing");
});
test("nonfinite and out-of-range reports never become quota",()=>{
  for (const value of [null,NaN,Infinity,-1,1.1]) {
    const q=quota(); q.groups[0].buckets[0].remaining_fraction=value;
    assert.equal(dashboardPools(q)[0].windows[0].remaining,null);
  }
});

test("an extra unknown window is partial despite complete recognized windows",()=>{
  const a=account(); a.quota.groups[0].buckets.push(bucket("alien",0.7));
  assert.equal(dashboardAccountState(a,now).quota,"partial");
});
test("cross-group bucket/window duplicates do not depend on coverage, group key or name",()=>{
  for (const reverse of [false,true]) {
    const q=quota();
    q.groups.push({key:"different-partial-key",display_name:"Different name",buckets:[structuredClone(q.groups[0].buckets[0])]});
    if (reverse) q.groups.reverse();
    const windows=dashboardPools(q).flatMap(p=>p.windows);
    assert.equal(windows.length,2); assert.equal(windows.filter(w=>w.window==="5h").length,1);
    assert.equal(windows.find(w=>w.window==="5h").remaining,80);
  }
});
test("cross-group conflicting values and reset times are unknown conflicts",()=>{
  for (const conflict of ["remaining_fraction","reset_time"]) {
    const q=quota(); const duplicate=structuredClone(q.groups[0].buckets[0]);
    duplicate[conflict]=conflict==="remaining_fraction" ? 0.2 : "2026-10-04T12:00:00Z";
    q.groups.push({key:"partial-other",display_name:"Other",buckets:[duplicate]});
    const windows=dashboardPools(q).flatMap(p=>p.windows);
    assert.equal(windows.length,2);
    assert.equal(windows.find(w=>w.window==="5h").remaining,null);
    assert.equal(windows.find(w=>w.window==="5h").conflict,true);
  }
});
test("equal names never merge distinct bucket identities",()=>{
  const q=quota(); q.groups.push({display_name:"Shared",buckets:[bucket("other-pool",0.3)]});
  assert.equal(dashboardPools(q).length,2);
  assert.equal(dashboardPools(q).flatMap(p=>p.windows).length,3);
});
test("sub-one-percent observed model quota is positive rather than exhausted",()=>{
  const a=account(); a.quota.groups=null;
  for (const remaining of [0.9,0.4]) {
    a.quota.models=[{name:"model",percentage:remaining,display_name:null,reset_time:"",inferred_bucket_id:null}];
    assert.equal(dashboardAccountState(a,now).quota,"reported");
    assert.equal(dashboardPools(a.quota)[0].windows[0].remaining,remaining);
  }
});
