import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";
const source = ts.transpileModule(
  readFileSync(
    new URL("../src/utils/menuBarQuota.ts", import.meta.url),
    "utf8",
  ),
  {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
  },
).outputText;
const {
  compactQuotaGroups,
  observedPercentage,
  validPercentage,
  lowestKnownQuota,
  isAccountSwitchable,
  isQuotaStale,
  resetTimestamp,
  quotaPages,
  pageSlice,
  pageSizeForHeight,
  summarizeAccounts,
  accountReadiness,
  quotaThreshold,
} = await import(
  `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
);
let passed = 0;
const test = (name, check) => {
  check();
  passed++;
  console.log(`PASS ${name}`);
};
test("unknown percentages remain unknown", () => {
  for (const value of [undefined, null, NaN, Infinity, -1, 101, "50"])
    assert.equal(validPercentage(value), null);
});
test("zero is a real known quota", () => assert.equal(validPercentage(0), 0));
test("missing observations and legacy ambiguous zeros are not exhausted", () => {
  assert.equal(observedPercentage(0), null);
  assert.equal(observedPercentage(0, false), null);
  assert.equal(observedPercentage(0, true), 0);
  assert.equal(observedPercentage(70, false), null);
});
test("read-only model fractions retain precision through legacy truncation/rounding", () => {
  for (const fraction of [0.009, 0.004]) {
    for (const percentage of [Math.trunc(fraction*100), Math.round(fraction*100)]) {
      const remaining = compactQuotaGroups({models:[{name:"model",percentage,percentage_known:true,observed_remaining_fraction:fraction,reset_time:""} ]})[0].rows[0].remaining;
      assert.ok(Math.abs(remaining-fraction*100)<1e-12);
      assert.ok(remaining>0);
    }
  }
  assert.equal(compactQuotaGroups({models:[{name:"legacy",percentage:0,percentage_known:true,reset_time:""}]})[0].rows[0].remaining,null);
});
test("fraction must lie within its actual range", () => {
  assert.equal(validPercentage(0.42, true), 42);
  assert.equal(validPercentage(1.2, true), null);
});
test("missing groups fall back to reported models only", () =>
  assert.deepEqual(
    compactQuotaGroups(
      { models: [{ name: "reported", percentage: 70, reset_time: "" }] },
      ["missing"],
    )[0].rows[0].remaining,
    70,
  ));
test("pinned model order is filtered without fabricating zeros", () =>
  assert.equal(
    compactQuotaGroups(
      {
        models: [
          { name: "a", percentage: 0, reset_time: "" },
          { name: "b", percentage: 50, reset_time: "" },
        ],
      },
      ["a"],
    ).length,
    1,
  ));
test("pool windows remain separate", () => {
  const groups = compactQuotaGroups({
    models: [],
    quota_groups: [
      {
        display_name: "Gemini",
        buckets: [
          {
            bucket_id: "1",
            window: "5h",
            remaining_fraction: 0.8,
            reset_time: "",
          },
          {
            bucket_id: "2",
            window: "weekly",
            remaining_fraction: 0.2,
            reset_time: "",
          },
        ],
      },
    ],
  });
  assert.equal(groups[0].rows.length, 2);
  assert.equal(groups[0].rows[1].remaining, 20);
});
test("forbidden quota never renders stale percentages", () =>
  assert.deepEqual(
    compactQuotaGroups({
      is_forbidden: true,
      models: [{ name: "a", percentage: 70 }],
    }),
    [],
  ));
test("no data produces no card", () =>
  assert.deepEqual(compactQuotaGroups(undefined), []));
test("lowest reported balance does not average independent pools", () =>
  assert.equal(
    lowestKnownQuota({
      quota: {
        models: [
          { name: "a", percentage: 50 },
          { name: "b", percentage: 10 },
        ],
      },
    }),
    10,
  ));
test("unknown account quota is not zero", () =>
  assert.equal(lowestKnownQuota({}), null));
test("disabled and currently blocked accounts cannot switch", () => {
  assert.equal(isAccountSwitchable({ disabled: true }), false);
  assert.equal(isAccountSwitchable({ validation_blocked: true }), false);
  assert.equal(
    isAccountSwitchable(
      { validation_blocked: true, validation_blocked_until: 1 },
      2000,
    ),
    true,
  );
});
test("quota freshness and missing timestamps", () => {
  assert.equal(isQuotaStale(undefined), true);
  assert.equal(isQuotaStale(100, 15, 200000), false);
  assert.equal(isQuotaStale(100, 15, 2000000), true);
});
test("invalid resets stay unavailable", () => {
  assert.equal(resetTimestamp("bad"), null);
  assert.equal(resetTimestamp(""), null);
  assert.ok(resetTimestamp("2026-10-01T12:00:00Z"));
});
test("quota pages show at most four rows without dropping later pools", () => {
  const groups = Array.from({ length: 3 }, (_, index) => ({
    name: `Pool ${index}`,
    rows: Array.from({ length: 3 }, (_, row) => ({
      id: `${index}-${row}`,
      label: "weekly",
      remaining: 50,
      resetTime: "",
    })),
  }));
  const pages = quotaPages(groups);
  assert.deepEqual(
    pages.map((page) => page.length),
    [4, 4, 1],
  );
  assert.equal(pages.flat().length, 9);
  assert.equal(pages[2][0].group, "Pool 2");
});
test("model fallback is paginated rather than silently truncated", () => {
  const groups = compactQuotaGroups({
    models: Array.from({ length: 7 }, (_, index) => ({
      name: `model-${index}`,
      percentage: 50,
      reset_time: "",
    })),
  });
  assert.deepEqual(
    quotaPages(groups).map((page) => page.length),
    [4, 3],
  );
});
test("large account lists use fixed size pages and clamp stale page indexes", () => {
  const accounts = Array.from({ length: 13 }, (_, index) => index);
  assert.deepEqual(pageSlice(accounts, 0), [0, 1, 2, 3]);
  assert.deepEqual(pageSlice(accounts, 3), [12]);
  assert.deepEqual(pageSlice(accounts, 999), [12]);
  assert.deepEqual(pageSlice(accounts, -4), [0, 1, 2, 3]);
  assert.deepEqual(pageSlice([], 4), []);
});
test("the compact panel has no scroll surfaces or decorative gradients", () => {
  const css = readFileSync(
    new URL("../src/components/menubar/MenuBarDashboard.css", import.meta.url),
    "utf8",
  );
  assert.doesNotMatch(css, /overflow(?:-y)?:\s*(auto|scroll)/);
  assert.doesNotMatch(css, /(?<!repeating-)linear-gradient/);
  assert.match(css, /overflow:\s*hidden/);
});
test("short display work areas reduce page size instead of scrolling or clipping", () => {
  assert.equal(pageSizeForHeight(480), 4);
  assert.equal(pageSizeForHeight(400), 3);
  assert.equal(pageSizeForHeight(330), 2);
  assert.equal(pageSizeForHeight(280), 1);
});

const overviewNow = Date.parse("2026-10-01T12:00:00Z");
const quotaAccount = (id, session = 0.8, weekly = 0.6) => ({
  id,
  email: `${id}@example.com`,
  quota: {
    last_updated: overviewNow / 1000 - 30,
    models: [],
    quota_groups: [
      {
        display_name: "Gemini Models",
        buckets: [
          {
            bucket_id: "session",
            window: "5h",
            remaining_fraction: session,
            remaining_fraction_known: true,
            reset_time: "2026-10-01T16:00:00Z",
          },
          {
            bucket_id: "week",
            window: "weekly",
            remaining_fraction: weekly,
            remaining_fraction_known: true,
            reset_time: "2026-10-07T12:00:00Z",
          },
        ],
      },
    ],
  },
});
test("overview counts all saved accounts and excludes stale, disabled and unknown quota", () => {
  const fresh = quotaAccount("fresh");
  const low = quotaAccount("low", 0.8, 0.05);
  const stale = quotaAccount("stale");
  stale.quota.last_updated -= 3600;
  const disabled = { ...quotaAccount("disabled"), disabled: true };
  const unknown = { id: "unknown", email: "unknown@example.com" };
  const result = summarizeAccounts(
    [fresh, low, stale, disabled, unknown],
    10,
    15,
    overviewNow,
  );
  assert.deepEqual(
    [
      result.total,
      result.healthy,
      result.low,
      result.unavailable,
      result.unknown,
    ],
    [5, 1, 1, 1, 2],
  );
  assert.deepEqual(
    [
      result.pools[0].total,
      result.pools[0].usable,
      result.pools[0].low,
      result.pools[0].unavailable,
      result.pools[0].unknown,
    ],
    [5, 1, 1, 1, 2],
  );
});
test("a healthy session cannot mask an exhausted weekly window", () => {
  assert.equal(
    summarizeAccounts([quotaAccount("low", 1, 0)], 10, 15, overviewNow).pools[0]
      .usable,
    0,
  );
});
test("missing quota windows are unverified instead of unlimited", () => {
  const partial = quotaAccount("partial");
  partial.quota.quota_groups[0].buckets.pop();
  const result = summarizeAccounts(
    [quotaAccount("complete"), partial],
    10,
    15,
    overviewNow,
  );
  assert.equal(result.pools[0].usable, 1);
  assert.equal(result.pools[0].unknown, 1);
  assert.equal(result.statuses.partial, "unknown");
  assert.equal(result.healthy, 1);
});
test("protection, validation and expired reset snapshots are never usable", () => {
  const protectedAccount = {
    ...quotaAccount("protected"),
    protected_models: ["gemini-3-pro-high"],
  };
  const verification = {
    ...quotaAccount("verification"),
    validation_blocked: true,
  };
  const expired = quotaAccount("expired");
  expired.quota.quota_groups[0].buckets[0].reset_time = "2026-10-01T11:59:00Z";
  const result = summarizeAccounts(
    [protectedAccount, verification, expired],
    10,
    15,
    overviewNow,
  );
  assert.equal(result.pools[0].usable, 0);
  assert.equal(result.unavailable, 1);
  assert.equal(result.unknown, 2);
});
test("group quotas and model fallback are not merged or summed", () => {
  const grouped = quotaAccount("grouped");
  const model = {
    id: "model",
    quota: {
      last_updated: overviewNow / 1000,
      models: [{ name: "Gemini Models", percentage: 70, reset_time: "" }],
    },
  };
  const result = summarizeAccounts([grouped, model], 10, 15, overviewNow);
  assert.equal(result.pools.length, 2);
  assert.deepEqual(
    result.pools.map((pool) => [pool.usable, pool.total]),
    [
      [1, 2],
      [1, 2],
    ],
  );
});
test("configured threshold is read without enabling automatic switching", () => {
  assert.equal(quotaThreshold(undefined), 10);
  assert.equal(quotaThreshold(150), 10);
  assert.equal(
    accountReadiness(quotaAccount("fifty", 0.5, 0.5), 50, 15, overviewNow),
    "low",
  );
  assert.equal(
    accountReadiness(quotaAccount("fifty", 0.5, 0.5), 10, 15, overviewNow),
    "healthy",
  );
});
// Account inspection and live/saved identity badges are exercised by the UI suite.
test("reopening a window cannot replay stale Dock preferences", () => {
  const desktop = readFileSync(new URL("../src-tauri/src/modules/desktop.rs", import.meta.url), "utf8");
  const show = desktop.slice(desktop.indexOf("pub fn show_main("), desktop.indexOf("pub fn open_app_page("));
  assert.doesNotMatch(show, /load_app_config|apply_dock_preference|set_activation_policy/);
  assert.match(show, /reconcile_dock_preference\(app\)/);
  assert.match(desktop, /reconcile_dock_preference\(window\.app_handle\(\)\)/);
  assert.match(desktop, /dock_error: std::sync::Mutex<Option<String>>/);
});
console.log(`${passed} tests passed`);
