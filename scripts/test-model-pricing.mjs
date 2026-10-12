import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import ts from 'typescript';
const source=ts.transpileModule(readFileSync(new URL('../src/utils/modelPricing.ts',import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.ES2022}}).outputText;
const {findModelPricing,estimateApiCost}=await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
const price={model:'gemini-2.5-flash',input:.3,output:2.5,cached:.03};
const snapshot={prices:[price]};
const model=name=>({model:name,input_tokens:1e6,output_tokens:1e6,cached_tokens:1e6});
const aliases=JSON.parse(readFileSync(new URL('../tests/fixtures/model-pricing-aliases.json',import.meta.url),'utf8'));
for(const [index,fixture] of aliases.entries())test(`shared Gemini pricing case ${index+1}: ${fixture.model}`,()=>{
 const prices={prices:fixture.prices.map((model,index)=>({model,input:.75*(index+1),output:3.75*(index+1),cached:.075*(index+1)}))};
 const matched=findModelPricing(fixture.model,prices);
 assert.equal(matched?.model??null,fixture.expected);
 const cost=estimateApiCost([model(fixture.model)],prices);
 assert.equal(cost.unpricedModels,fixture.expected===null?1:0);
 assert.equal(cost.pricedModels,fixture.expected===null?0:1);
 if(matched)assert(Math.abs(cost.usd-(matched.input+matched.output+matched.cached))<1e-9);
});
test('exact aliases work without borrowing neighbouring model prices',()=>{
 assert.equal(findModelPricing('Gemini 2.5 Flash-n',snapshot),price);
 for(const name of ['gemini-2.5-flash-lite','gemini-2.5','claude-sonnet-4.6',''])assert.equal(findModelPricing(name,snapshot),undefined);
});
test('ambiguous and invalid rates are unpriced',()=>{
 assert.equal(findModelPricing(price.model,{prices:[price,{...price}]}),undefined);
 for(const input of [-1,NaN,Infinity])assert.equal(findModelPricing(price.model,{prices:[{...price,input}]}),undefined);
});
test('Claude thinking aliases match only their exact model rates',()=>{
 const opus={model:'Claude Opus 4.6',input:5,output:25,cached:.5};
 const prices={prices:[opus]};
 for(const name of ['claude-opus-4-6-thinking','Claude Opus 4.6 Thinking-n','claude-opus-4.6']){
  assert.equal(findModelPricing(name,prices),opus);
  assert.deepEqual(estimateApiCost([model(name)],prices),{usd:30.5,pricedModels:1,unpricedModels:0});
 }
 for(const name of ['claude-opus-4-5-thinking','claude-opus-4-6-fast','claude-opus-4-6-thinking-exp','claude-sonnet-4-6-thinking'])assert.equal(findModelPricing(name,prices),undefined);
 assert.equal(findModelPricing('claude-opus-4-6-thinking',{prices:[opus,{...opus,model:'claude-opus-4-6-thinking'}]}),undefined);
});
test('input, output and cache are charged once and partial estimates exclude unknown models',()=>{
 assert.deepEqual(estimateApiCost([model(price.model),model('unknown')],snapshot),{usd:2.83,pricedModels:1,unpricedModels:1});
});
test('unknown cost differs from known zero and no activity',()=>{
 assert.deepEqual(estimateApiCost([model('unknown')],snapshot),{usd:0,pricedModels:0,unpricedModels:1});
 assert.deepEqual(estimateApiCost([model(price.model)],{prices:[{...price,input:0,output:0,cached:0}]}),{usd:0,pricedModels:1,unpricedModels:0});
 assert.deepEqual(estimateApiCost([],null),{usd:0,pricedModels:0,unpricedModels:0});
});
