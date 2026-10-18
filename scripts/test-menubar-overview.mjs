import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';
const compile = path => ts.transpileModule(readFileSync(new URL(path, import.meta.url), 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const url = source => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const dependency = url(compile('../src/utils/accountDashboard.ts'));
const config = url(compile('../src/types/config.ts'));
const source = compile('../src/utils/menuBarOverview.ts').replaceAll("'./accountDashboard'", JSON.stringify(dependency)).replaceAll("'../types/config'", JSON.stringify(config));
const { menuBarAccount, aggregateMenuBar, aggregateArgon, quotaDisplay, quotaTone } = await import(url(source));
const now = Date.parse('2026-10-04T12:00:00Z');
const clone = value => structuredClone(value);
function account(id, gemini = [.8,.6], other = [.4,.2]) {
  return { id, email: `${id}@example.com`, name:null, custom_label:null, read_status:'loaded', read_error:null,
    disabled:false,validation_blocked:false,validation_blocked_until:null,protected_models:[],
    quota:{ last_updated:now/1000-30,is_forbidden:false,subscription_tier:'PRO',provenance:'observed',models:[],
      groups:[['Gemini Models',gemini],['Claude/GPT Models',other]].map(([display_name,values],i)=>({display_name,buckets:values.map((remaining_fraction,j)=>({bucket_id:`pool-${i}-${j}`,window:j?'weekly':'5h',remaining_fraction,reset_time:new Date(now+(j?86400000:3600000)).toISOString()}))})) } };
}
let passed=0;
function test(name,fn){fn();console.log(`PASS ${name}`);passed++;}
const views = (saved,interval=15) => saved.map(a=>menuBarAccount(a,now,interval));
test('Argon is a separate mean and does not dilute shared Gemini or require five-hour data',()=>{
  const a=account('a',[.8,.9]);
  a.quota.groups.unshift({display_name:'Gemini 4 Argon',buckets:[{bucket_id:'gemini-argon-wtu-weekly',window:'weekly',remaining_fraction:.1,reset_time:new Date(now+86400000).toISOString()}]});
  const b=account('b');
  assert.equal(menuBarAccount(a,now).windows.weekly.gemini.remaining,90);
  assert.equal(menuBarAccount(a,now).argonWeekly.remaining,10);
  assert.deepEqual(aggregateArgon(views([a,b])),{remaining:10,used:90,covered:1,total:1});
  a.quota.groups[0].buckets[0].remaining_fraction=null;
  assert.deepEqual(aggregateArgon(views([a,b])),{remaining:null,used:null,covered:0,total:1});
  a.quota.groups[0].buckets[0].remaining_fraction=0;
  assert.equal(menuBarAccount(a,now).argonWeekly.remaining,0);
  a.quota.last_updated-=3600;
  assert.equal(menuBarAccount(a,now).argonWeekly.remaining,null);
});
test('two scopes and two periods remain independent; all uses an equal-weight mean',()=>{
  const data=views([account('a'),account('b',[1,.8],[.6,.4])]);
  assert.equal(aggregateMenuBar(data,'gemini','5h').remaining,90);
  assert.equal(aggregateMenuBar(data,'other','5h').remaining,50);
  assert.equal(aggregateMenuBar(data,'all','5h').remaining,70);
  assert.equal(aggregateMenuBar(data,'all','weekly').remaining,50);
});
test('low and known zero quotas remain in the mean and lower usable counts',()=>{
  const data=views([account('zero',[0,0],[0,0]),account('full',[1,1],[1,1])]);
  assert.deepEqual(aggregateMenuBar(data,'all','5h'),{remaining:50,usable:1,covered:2,total:2});
});
test('shared buckets repeated under model groups do not multiply their weight',()=>{
  const a=account('a');a.quota.groups.push(clone(a.quota.groups[0]));
  const b=account('b',[1,1],[1,1]);
  assert.equal(aggregateMenuBar(views([a,b]),'all','5h').remaining,80);
});
test('conflicting duplicated observations and cross-family ownership fail closed',()=>{
  const a=account('a');const duplicate=clone(a.quota.groups[0]);duplicate.buckets[0].remaining_fraction=.3;a.quota.groups.push(duplicate);
  assert.equal(menuBarAccount(a,now).windows['5h'].gemini.reason,'conflict');
  const b=account('b');b.quota.groups[1].buckets[0]=clone(b.quota.groups[0].buckets[0]);
  assert.equal(aggregateMenuBar(views([b]),'all','5h').remaining,null);
});
test('forbidden, disabled, blocked, protected, unreadable and stale accounts are excluded',()=>{
  const good=account('good');const excluded=['disabled','blocked','protected','unreadable','stale','forbidden'].map(key=>{
    const a=account(key);
    if(key==='disabled') a.disabled=true;
    if(key==='blocked') a.validation_blocked=true;
    if(key==='protected') a.protected_models=['gemini'];
    if(key==='unreadable') a.read_status='failed';
    if(key==='stale') a.quota.last_updated-=3600;
    if(key==='forbidden') a.quota.is_forbidden=true;
    return a;
  });
  assert.deepEqual(aggregateMenuBar(views([good,...excluded]),'all','5h'),{remaining:60,usable:1,covered:1,total:7});
  assert.equal(menuBarAccount(excluded[0],now).switchable,false);
});
test('missing session data cannot borrow weekly or model quota',()=>{
  const a=account('a');a.quota.groups.forEach(g=>g.buckets=g.buckets.filter(b=>b.window==='weekly'));
  a.quota.models=[{name:'gemini-flash',percentage:100,display_name:null,reset_time:new Date(now+3600000).toISOString(),inferred_bucket_id:null}];
  assert.equal(aggregateMenuBar(views([a]),'all','5h').remaining,null);
  assert.equal(aggregateMenuBar(views([a]),'all','weekly').remaining,40);
});
test('unknown fractions, unnamed buckets, invalid and expired resets never become capacity',()=>{
  for(const tweak of [b=>b.remaining_fraction=null,b=>b.remaining_fraction=-.1,b=>b.remaining_fraction=1.1,b=>b.bucket_id='',b=>b.reset_time='',b=>b.reset_time='bad',b=>b.reset_time=new Date(now-1).toISOString()]) {
    const a=account('a');tweak(a.quota.groups[0].buckets[0]);
    assert.equal(aggregateMenuBar(views([a]),'all','5h').remaining,null);
  }
});
test('all-series requires complete selected-family coverage',()=>{
  const a=account('a');a.quota.groups.pop();
  assert.equal(aggregateMenuBar(views([a]),'gemini','5h').remaining,80);
  assert.equal(aggregateMenuBar(views([a]),'all','5h').covered,0);
});
test('future cache timestamps and invalid freshness settings cannot bypass expiry',()=>{
  const a=account('future');a.quota.last_updated=now/1000+3600;
  assert.equal(aggregateMenuBar(views([a]),'all','5h').remaining,null);
  const b=account('stale');b.quota.last_updated-=3600;
  assert.equal(aggregateMenuBar(views([b],Infinity),'all','5h').remaining,null);
});
test('fractional values retain precision while display rounds to integer percentages',()=>{
  const a=account('a',[.004,.6]);
  assert.equal(aggregateMenuBar(views([a]),'gemini','5h').remaining,.4);
  assert.equal(quotaDisplay(.4),'0%');assert.equal(quotaDisplay(99.6),'100%');
  assert.equal(quotaDisplay(0),'0%');assert.equal(quotaDisplay(null),'—');
});
test('normalization and aggregation do not mutate saved observations',()=>{
  const a=account('a');const original=JSON.stringify(a);aggregateMenuBar(views([a]),'all','weekly');assert.equal(JSON.stringify(a),original);
});
test('quota color boundaries and customized cutoffs',()=>{
  for (const [value,tone] of [[19,'critical'],[20,'warning'],[60,'warning'],[61,'healthy'],[null,'unknown'],[NaN,'unknown']]) assert.equal(quotaTone(value,{quota_scope:'all'}),tone);
  assert.equal(quotaTone(70,{quota_scope:'all',green_above:80,red_below:30}),'warning');
});
console.log(`Menu bar aggregation: ${passed} passed`);
