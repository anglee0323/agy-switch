/* Reversible translation of Antigravity's built-in UI. Never translates values,
 * conversation bodies, code, terminals, artifacts or embedded web pages. */
(function createAppLocalization(host, config) {
  'use strict';
  const KEY = '__ANTIGRAVITY_TOOLS_LOCALIZATION__';
  const brand = 'antigravity-tools-scoped-localization-v1';
  const doc = host.document;
  const dictionary = config.dictionary.exact;
  const forbidden = 'pre,code,textarea,[contenteditable],.markdown,.markdown-body,.prose,.monaco-editor,.xterm,[data-cascade-id],[data-message-id],[data-user-content],[translate="no"],iframe,script,style';
  const chrome = '[data-testid="settings-button"],[data-testid^="permissions-"],nav,header,[role="menu"],[role="listbox"],[role="tablist"],[role="tooltip"],[role="dialog"],.settings-modal-container';
  const sidebarControls = '[data-testid="settings-button"],[data-testid="new-conversation-button"],[data-testid="history-button"],[data-testid="automations-button"],a[href="/customizations"],a[href="/projects"]';
  const attributes = ['placeholder','title','aria-label','aria-description'];
  let active = false, observer, timer, lease;
  const records = new Map();
  function report(status = active ? 'applied' : 'supported') {
    return {status, active, translated:records.size, labelCount:records.size, awaitingScope:false};
  }
  function translated(raw) {
    const key = raw.trim();
    const value = Object.prototype.hasOwnProperty.call(dictionary, key) ? dictionary[key] :
      /^See \d+ more skills and rules$/.test(key) ? key.replace(/^See (\d+) more skills and rules$/, '查看另外 $1 项技能和规则') :
      /^Show \d+ breakdowns?$/.test(key) ? key.replace(/^Show (\d+) breakdowns?$/, '查看 $1 项明细') :
      /^See \d+ more$/.test(key) ? key.replace(/^See (\d+) more$/, '查看另外 $1 项') :
      /^Install .+$/.test(key) ? key.replace(/^Install /, '安装 ') :
      /^Select (project|model), current: .+$/.test(key) ? key.replace(/^Select (project|model), current: /, (_, kind) => `选择${kind==='project'?'项目':'模型'}，当前：`) :
      /^You have used some of your weekly limit, it will fully refresh in [\d, a-z]+\.$/.test(key) ? key.replace(/^You have used some of your weekly limit, it will fully refresh in (.+)\.$/, (_, duration) => `本周额度已使用部分，将在 ${duration.replace(/(\d+) days?/g,'$1 天').replace(/(\d+) hours?/g,'$1 小时').replace(/(\d+) minutes?/g,'$1 分钟')}后完全重置。`) :
      /^\d+ (skills|rules|commands|plugins)$/.test(key) ? key.replace(/^(\d+) (skills|rules|commands|plugins)$/, (_, n, kind) => `${n} 项${({skills:'技能',rules:'规则',commands:'命令',plugins:'插件'})[kind]}`) : null;
    return value && value !== key ? raw.replace(key, value) : null;
  }
  function eligible(element, attribute) {
    if (attribute === 'aria-label' && element?.matches('[contenteditable][aria-label="Message input"],[contenteditable][aria-label="消息输入框"]') && !element.closest('pre,code,.monaco-editor,iframe')) return true;
    if (!element || element.closest(forbidden)) return false;
    if (element.closest('.font-mono') && !element.closest('[role="combobox"],[role="option"],[role="menuitem"]')) return false;
    if (!attribute && element.closest('[data-project-card],[data-project-name],[data-testid="project-selector-trigger"],[data-testid="settings-nav-item-Account"]')) return false;
    if (element.matches('span[title]') && element.getAttribute('title') === element.textContent.trim()) return false;
    const item = element.closest('[data-testid^="settings-nav-item-"]');
    if (item) {
      // Project/workspace names use the same component as fixed settings links.
      // Use the section heading, never the name itself, to distinguish them.
      const container = item.closest('.settings-modal-container') || doc;
      let section = '';
      for (const candidate of container.querySelectorAll('h1,h2,[data-testid^="settings-nav-item-"]')) {
        if (candidate === item) break;
        if (candidate.matches('h1,h2')) section = candidate.textContent.trim();
      }
      if (['Projects','项目','Workspaces','工作区'].includes(section) || item.dataset.testid === 'settings-nav-item-Account') return false;
    }
    if (!attribute && element.closest('h2')?.querySelector('button[aria-label^="Edit "][aria-label$=" name"],button[aria-label^="编辑"][aria-label$="名称"]') && !element.closest('button')) return false;
    if (!attribute && element.closest('input,select,[role="textbox"],[role="searchbox"]')) return false;
    if (element.closest(sidebarControls)) return true;
    if (element.matches('p.pointer-events-none') && /^(Ask anything, @ to mention|Message subagent, @ to mention|输入问题，@ 引用内容|向子智能体发送消息，@ 引用内容)/.test(element.textContent.trim())) return true;
    if (element.closest('[aria-label="Select environment"],[aria-label="选择环境"]')) return true;
    if (element.closest('[role="navigation"][aria-label="Sidebar"],[role="navigation"][aria-label="侧边栏"]')) {
      if (attribute) return true;
      return !!element.closest('h1,h2,h3,h4') && !element.closest('[data-project-card]');
    }
    if (element.closest(chrome)) return true;
    // Sidebar destination pages contain UI descriptions as well as controls.
    // Names and descriptions supplied by users remain outside static-label fields.
    const route = host.location?.pathname || '';
    const pageHeading = [...doc.querySelectorAll('h1')].some(h => !h.closest('.settings-modal-container') && ['Customizations','自定义项','Automations','自动化','Conversation History','对话历史','History','历史','Projects','项目'].includes(h.textContent.trim()));
    if (/^\/(settings-standalone|customizations|automations|sidecars|history|projects)(\/|$)/.test(route) || pageHeading) {
      if (element.matches('.cursor-pointer') && ['Customizations','自定义项'].includes(element.textContent.trim())) return true;
      return !!element.closest('button,label,h1,h2,h3,h4,[role="tab"],[role="switch"],input,[data-testid="empty-state"],.text-muted-foreground');
    }
    // Only fixed navigation links and control hints elsewhere in a conversation.
    return !!element.closest('a[href="/history"],a[href="/automations"],a[href="/customizations"],a[href="/projects"]') ||
      (attribute && !!element.closest('button'));
  }
  function applyValue(node, attribute, raw) {
    let map = records.get(node);
    const record = map?.get(attribute);
    if (record && raw === record.target) return;
    if (record) { map.delete(attribute); if (!map.size) records.delete(node); }
    const element = attribute ? node : node.parentElement;
    if (!eligible(element, attribute)) return;
    const target = raw.trim()==='Type' && element.parentElement?.textContent.includes('/') ? raw.replace('Type','输入') : translated(raw);
    if (!target) return;
    if (!map) map = new Map();
    map.set(attribute,{original:raw,target}); records.set(node,map);
    if (attribute) node.setAttribute(attribute,target); else node.nodeValue=target;
  }
  function sweep() {
    timer = undefined;
    if (!active) return;
    const walker = doc.createTreeWalker(doc.body || doc.documentElement,4);
    while (walker.nextNode()) { const n=walker.currentNode; if (n.nodeValue?.trim()) applyValue(n,'',n.nodeValue); }
    for (const element of doc.querySelectorAll('[placeholder],[title],[aria-label],[aria-description]')) {
      for (const attribute of attributes) { const value=element.getAttribute(attribute); if (value) applyValue(element,attribute,value); }
    }
    for (const [node,map] of records) {
      if (!node.isConnected) {
        for (const [attribute,record] of map) {
          const current=attribute ? node.getAttribute(attribute) : node.nodeValue;
          if (current===record.target) { if (attribute) node.setAttribute(attribute,record.original); else node.nodeValue=record.original; }
        }
        records.delete(node); continue;
      }
      for (const [attribute,record] of map) {
        if (!eligible(attribute ? node : node.parentElement,attribute)) {
          const current=attribute ? node.getAttribute(attribute) : node.nodeValue;
          if (current===record.target) { if (attribute) node.setAttribute(attribute,record.original); else node.nodeValue=record.original; }
          map.delete(attribute);
        }
      }
      if (!map.size) records.delete(node);
    }
  }
  function dispose() {
    active=false; observer?.disconnect(); host.clearTimeout(timer); host.clearTimeout(lease);
    for (const [node,map] of records) for (const [attribute,record] of map) {
      const current=attribute ? node.getAttribute(attribute) : node.nodeValue;
      if (current===record.target) { if (attribute) node.setAttribute(attribute,record.original); else node.nodeValue=record.original; }
    }
    records.clear(); return report('disposed');
  }
  function renewLease() { host.clearTimeout(lease); lease=host.setTimeout(dispose,15000); return report(); }
  function apply() {
    if (!active) {
      active=true;
      observer=new host.MutationObserver(() => { if (active && timer===undefined) timer=host.setTimeout(sweep,30); });
      observer.observe(doc.documentElement,{subtree:true,childList:true,characterData:true,attributes:true,attributeFilter:[...attributes,'class','role','contenteditable','translate']});
    }
    sweep(); renewLease(); return report();
  }
  // Reapplying refreshes a live controller rather than creating competing observers.
  const previous=host[KEY];
  if (previous?.brand===brand && previous.active()) { previous.renewLease(); return previous; }
  const api={brand,probe:()=>report(),apply,renewLease,dispose,active:()=>active};
  host[KEY]=api; return api;
})
