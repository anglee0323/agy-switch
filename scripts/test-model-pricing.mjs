import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import ts from 'typescript';
const source=ts.transpileModule(readFileSync(new URL('../src/utils/modelPricing.ts',import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.ES2022}}).outputText;
const {findModelPricing,estimateApiCost}=await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
const price={model:'gemini-2.5-flash',input:.3,output:2.5,cached:.03};
const snapshot={prices:[price]};
const model=name=>({model:name,input_tokens:1e6,output_tokens:1e6,cached_tokens:1e6});
test('explicit EXP-A estimate mapping does not absorb other versions or variants',()=>{
 const flash={...price,model:'gemini-3.8-flash'};
 const prices={prices:[flash]};
 assert.equal(findModelPricing('GEMINI-3.8-FLASH-EXP-A',prices),flash);
 for(const name of ['gemini-3.8-flash-exp-b','gemini-3.7-flash','gemini-3.8-flash-lite'])assert.equal(findModelPricing(name,prices),undefined);
});
test('exact aliases work without borrowing neighbouring model prices',()=>{
 assert.equal(findModelPricing('Gemini 2.5 Flash-n',snapshot),price);
 for(const name of ['gemini-2.5-flash-lite','gemini-2.5','claude-sonnet-4.6',''])assert.equal(findModelPricing(name,snapshot),undefined);
});
test('ambiguous and invalid rates are unpriced',()=>{
 assert.equal(findModelPricing(price.model,{prices:[price,{...price}]}),undefined);
 for(const input of [-1,NaN,Infinity])assert.equal(findModelPricing(price.model,{prices:[{...price,input}]}),undefined);
});
test('input, output and cache are charged once and partial estimates exclude unknown models',()=>{
 assert.deepEqual(estimateApiCost([model(price.model),model('unknown')],snapshot),{usd:2.83,pricedModels:1,unpricedModels:1});
});
test('unknown cost differs from known zero and no activity',()=>{
 assert.deepEqual(estimateApiCost([model('unknown')],snapshot),{usd:0,pricedModels:0,unpricedModels:1});
 assert.deepEqual(estimateApiCost([model(price.model)],{prices:[{...price,input:0,output:0,cached:0}]}),{usd:0,pricedModels:1,unpricedModels:0});
 assert.deepEqual(estimateApiCost([],null),{usd:0,pricedModels:0,unpricedModels:0});
});
