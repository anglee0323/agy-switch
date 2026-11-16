import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const source = ts.transpileModule(
  readFileSync(new URL('../src/utils/modelCategory.ts', import.meta.url), 'utf8'),
  { compilerOptions: { module: ts.ModuleKind.ES2022 } },
).outputText;
const {
  categorizeModel,
  getCanonicalModelKey,
  getCanonicalModelDisplayName,
  getCanonicalModelSublabel,
  getModelProtectionKey,
  findQuotaModel,
  ensurePinnedImageSelector,
} = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);

test('Gemini 4 effort tiers fold into their canonical model', () => {
  for (const id of ['gemini-4-carbon-low', 'gemini-4-carbon-medium', 'gemini-4-carbon-high', 'gemini-4-carbon-disabled', 'gemini-4-carbon']) {
    assert.equal(getCanonicalModelKey(id), 'gemini-4-carbon', id);
  }
  for (const id of ['gemini-4-argon-low', 'gemini-4-argon-medium', 'gemini-4-argon-high', 'gemini-4-argon']) {
    assert.equal(getCanonicalModelKey(id), 'gemini-4-argon', id);
  }
  for (const id of ['gemini-4-hydrogen-low', 'gemini-4-hydrogen-medium', 'gemini-4-hydrogen-high', 'gemini-4-hydrogen-xhigh', 'gemini-4-hydrogen']) {
    assert.equal(getCanonicalModelKey(id), 'gemini-4-hydrogen', id);
  }
  assert.equal(getCanonicalModelDisplayName('gemini-4-carbon-high'), 'Gemini 4 Carbon');
  assert.equal(getCanonicalModelDisplayName('gemini-4-argon-high'), 'Gemini 4 Argon');
  assert.equal(getCanonicalModelDisplayName('gemini-4-hydrogen-xhigh'), 'Gemini 4 Hydrogen');
  assert.equal(getCanonicalModelSublabel('gemini-4-argon'), 'Shared quota pool: Low / Medium / High');
  assert.equal(getCanonicalModelSublabel('gemini-4-hydrogen'), 'Shared quota pool: Low / Medium / High / Extra');
});

test('Orchestrator identities stay separate from their planner models', () => {
  assert.equal(getCanonicalModelKey('gemini-pro-orchestrator'), 'gemini-pro-orchestrator');
  assert.equal(getCanonicalModelKey('gemini-pro-orchestrator-g1-pro'), 'gemini-pro-orchestrator-argon');
  assert.equal(getCanonicalModelKey('gemini-pro-orchestrator-argon'), 'gemini-pro-orchestrator-argon');
  assert.equal(getCanonicalModelDisplayName('gemini-pro-orchestrator'), 'Orchestrator (Carbon)');
  assert.equal(getCanonicalModelDisplayName('gemini-pro-orchestrator-argon'), 'Orchestrator (Argon)');
  assert.notEqual(getCanonicalModelKey('gemini-pro-orchestrator'), getCanonicalModelKey('gemini-4-carbon'));
  assert.notEqual(getCanonicalModelKey('gemini-pro-orchestrator-argon'), getCanonicalModelKey('gemini-4-argon'));
});

test('Claude 5.5 tiers stay on 5.5 instead of collapsing into 4.6', () => {
  for (const id of ['claude-sonnet-5-5', 'claude-sonnet-5-5-low', 'claude-sonnet-5-5-medium', 'claude-sonnet-5-5-high', 'claude-sonnet-5.5-thinking']) {
    assert.equal(getCanonicalModelKey(id), 'claude-sonnet-5-5', id);
  }
  for (const id of ['claude-opus-5-5', 'claude-opus-5-5-low', 'claude-opus-5-5-medium', 'claude-opus-5-5-high', 'claude-opus-5.5-thinking']) {
    assert.equal(getCanonicalModelKey(id), 'claude-opus-5-5', id);
  }
  assert.equal(getCanonicalModelKey('claude-opus-4-6-thinking'), 'claude-opus-4-6-thinking');
  assert.equal(getCanonicalModelKey('claude-sonnet-4-6'), 'claude-sonnet-4-6');
  assert.equal(getCanonicalModelKey('claude-opus-5'), 'claude-opus-5');
  assert.equal(getCanonicalModelKey('claude-sonnet-5-high'), 'claude-sonnet-5');
});

test('image variants never collapse into a chat Flash or Pro family', () => {
  for (const id of ['gemini-3.1-flash-image', 'gemini-3.1-flash-lite-image', 'gemini-3.8-flash-image', 'gemini-3-pro-image', 'gemini-2.5-flash-image']) {
    assert.equal(getCanonicalModelKey(id), 'gemini-3.1-flash-image', id);
  }
});

test('catalog categories keep Carbon, Argon, and Hydrogen in the Gemini families', () => {
  assert.equal(categorizeModel('gemini-4-carbon-high'), 'gemini-pro');
  assert.equal(categorizeModel('gemini-4-argon-high'), 'gemini-pro');
  assert.equal(categorizeModel('gemini-4-hydrogen-xhigh'), 'gemini-flash');
  assert.equal(categorizeModel('gemini-pro-orchestrator'), 'gemini-pro');
  assert.equal(categorizeModel('gemini-pro-orchestrator-argon'), 'gemini-pro');
  assert.equal(categorizeModel('claude-opus-5-5'), 'claude');
  assert.equal(getModelProtectionKey('gemini-4-carbon'), 'gemini-3-pro-high');
  assert.equal(getModelProtectionKey('gemini-4-argon'), 'gemini-3-pro-high');
  assert.equal(getModelProtectionKey('gemini-4-hydrogen'), 'gemini-3-flash');
});

test('quota fallbacks prefer the current Gemini 4 and Claude 5.5 models', () => {
  const models = [
    { name: 'gemini-4-carbon-high' },
    { name: 'gemini-4-hydrogen-medium' },
    { name: 'claude-sonnet-5-5-high' },
    { name: 'claude-opus-5-5' },
  ];
  assert.equal(findQuotaModel(models, 'gemini-pro')?.name, 'gemini-4-carbon-high');
  assert.equal(findQuotaModel(models, 'gemini-flash')?.name, 'gemini-4-hydrogen-medium');
  assert.equal(findQuotaModel(models, 'claude')?.name, 'claude-opus-5-5');
  assert.deepEqual(ensurePinnedImageSelector(['gemini-4-carbon']), ['gemini-4-carbon', 'gemini-3.1-flash-image']);
});
