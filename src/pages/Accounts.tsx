

import {
  Download,
  Calendar,
  Clock,
  LayoutGrid,
  List,
  RefreshCw,
  Search,
  Trash2,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import AccountGrid from "../components/accounts/AccountGrid";
import AccountTable from "../components/accounts/AccountTable";
import AddAccountDialog from "../components/accounts/AddAccountDialog";
import ModalDialog from "../components/common/ModalDialog";
import Pagination from "../components/common/Pagination";
import { showToast } from "../components/common/ToastContainer";
import { useAccountStore } from "../stores/useAccountStore";
import { cn } from "../utils/cn";
import { useTranslation } from "react-i18next";
import { save } from "@tauri-apps/plugin-dialog";
import { format } from "date-fns";
import { saveAccountExport } from "../services/accountService";
import type { Account } from "../types/account";
import { getAccountTier, type AccountTier } from "../utils/accountTier";

type FilterType = "all" | AccountTier;
type ViewMode = "list" | "grid";
export type QuotaWindow = "5h" | "weekly";


function Accounts() {
  const { t } = useTranslation();
  const {
    accounts,
    currentAccount,
    fetchAccounts,
    fetchCurrentAccount,
    addAccount,
    deleteAccounts,
    switchAccount,
    loading,
    refreshQuota,
    reorderAccounts,
    updateAccountLabel,
  } = useAccountStore();
  const [searchQuery, setSearchQuery] = useState('');
  const [filter, setFilter] = useState<FilterType>('all');
  const [isSearchExpanded, setIsSearchExpanded] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const [viewMode, setViewMode] = useState<ViewMode>(() => {
    const saved = localStorage.getItem('accounts_view_mode');
    return (saved === 'list' || saved === 'grid') ? saved : 'list';
  });

  const [quotaWindow, setQuotaWindow] = useState<QuotaWindow>(() => {
    const saved = localStorage.getItem('accounts_quota_window');
    return (saved === '5h' || saved === 'weekly') ? saved : '5h';
  });

  // Save view mode preference
  useEffect(() => {
    localStorage.setItem('accounts_view_mode', viewMode);
  }, [viewMode]);

  // Save quota window preference
  useEffect(() => {
    localStorage.setItem('accounts_quota_window', quotaWindow);
  }, [quotaWindow]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [isBatchDelete, setIsBatchDelete] = useState(false);
  const [accountToDelete, setAccountToDelete] = useState<Account | null>(null);
  const [accountToEditLabel, setAccountToEditLabel] = useState<Account | null>(null);
  const [labelDraft, setLabelDraft] = useState('');
  const [isSavingLabel, setIsSavingLabel] = useState(false);
  const [refreshingIds, setRefreshingIds] = useState<Set<string>>(new Set());
  const containerRef = useRef<HTMLDivElement>(null);
  const [containerSize, setContainerSize] = useState({ width: 0, height: 0 });

  useEffect(() => {
    if (!containerRef.current) return;
    const resizeObserver = new ResizeObserver((entries) => {
      for (let entry of entries) {
        setContainerSize({
          width: entry.contentRect.width,
          height: entry.contentRect.height,
        });
      }
    });
    resizeObserver.observe(containerRef.current);
    return () => resizeObserver.disconnect();
  }, []);

  // Pagination State
  const [currentPage, setCurrentPage] = useState(1);
  const [localPageSize, setLocalPageSize] = useState<number | null>(() => {
    const saved = localStorage.getItem("accounts_page_size");
    return saved ? parseInt(saved) : null;
  }); // 本地分页大小状态

  // Save page size preference
  useEffect(() => {
    if (localPageSize !== null) {
      localStorage.setItem("accounts_page_size", localPageSize.toString());
    }
  }, [localPageSize]);

  // 动态计算分页条数
  const ITEMS_PER_PAGE = useMemo(() => {
    // 优先使用本地设置的分页大小
    if (localPageSize && localPageSize > 0) {
      return localPageSize;
    }

    // 回退到原有的动态计算逻辑
    if (!containerSize.height) return viewMode === "grid" ? 6 : 8;

    if (viewMode === "list") {
      const headerHeight = 36; // 缩深后的表头高度
      const rowHeight = 72; // 包含多行模型信息后的实际行高
      // 计算能容纳多少行, 默认最低 10 行
      const autoFitCount = Math.floor(
        (containerSize.height - headerHeight) / rowHeight,
      );
      return Math.max(10, autoFitCount);
    } else {
      const cardHeight = 180; // AccountCard 实际高度 (含间距)
      const gap = 16; // gap-4

      // 匹配 Tailwind 断点逻辑
      let cols = 1;
      if (containerSize.width >= 1200)
        cols = 4; // xl (约为 1280 左右)
      else if (containerSize.width >= 900)
        cols = 3; // lg (约为 1024 左右)
      else if (containerSize.width >= 600) cols = 2; // md (约为 768 左右)

      const rows = Math.max(
        1,
        Math.floor((containerSize.height + gap) / (cardHeight + gap)),
      );
      return cols * rows;
    }
  }, [localPageSize, containerSize, viewMode]);

  useEffect(() => {
    fetchAccounts();
    fetchCurrentAccount();
  }, [fetchAccounts, fetchCurrentAccount]);

  // Reset pagination when view mode changes to avoid empty pages or confusion
  useEffect(() => {
    setCurrentPage(1);
  }, [viewMode]);

  // 搜索过滤逻辑
  const searchedAccounts = useMemo(() => {
    if (!searchQuery) return accounts;
    const lowQuery = searchQuery.toLowerCase();
    return accounts.filter((a) => a.email.toLowerCase().includes(lowQuery));
  }, [accounts, searchQuery]);

  // 计算各筛选状态下的数量 (基于搜索结果)
  const filterCounts = useMemo(() => {
    const counts = { all: searchedAccounts.length, pro: 0, ultra: 0, free: 0, unknown: 0 };
    searchedAccounts.forEach(account => { counts[getAccountTier(account)]++; });
    return counts;
  }, [searchedAccounts]);

  // 过滤和搜索最终结果
  const filteredAccounts = useMemo(() => {
    return filter === "all" ? searchedAccounts
      : searchedAccounts.filter(account => getAccountTier(account) === filter);
  }, [searchedAccounts, filter]);

  // Pagination Logic
  const paginatedAccounts = useMemo(() => {
    const startIndex = (currentPage - 1) * ITEMS_PER_PAGE;
    return filteredAccounts.slice(startIndex, startIndex + ITEMS_PER_PAGE);
  }, [filteredAccounts, currentPage, ITEMS_PER_PAGE]);

  const handlePageChange = (page: number) => {
    setCurrentPage(page);
  };

  // 清空选择当过滤改变 并重置分页
  useEffect(() => {
    setSelectedIds(new Set());
    setCurrentPage(1);
  }, [filter, searchQuery]);

  const handleToggleSelect = (id: string) => {
    const newSet = new Set(selectedIds);
    if (newSet.has(id)) {
      newSet.delete(id);
    } else {
      newSet.add(id);
    }
    setSelectedIds(newSet);
  };

  const handleToggleAll = () => {
    // 全选当前页的所有项
    const currentIds = paginatedAccounts.map((a) => a.id);
    const allSelected = currentIds.every((id) => selectedIds.has(id));

    const newSet = new Set(selectedIds);
    if (allSelected) {
      currentIds.forEach((id) => newSet.delete(id));
    } else {
      currentIds.forEach((id) => newSet.add(id));
    }
    setSelectedIds(newSet);
  };

  const handleAddAccount = async (email: string, refreshToken: string) => {
    await addAccount(email, refreshToken);
  };

  const [switchingAccountId, setSwitchingAccountId] = useState<string | null>(
    null,
  );

  const handleSwitch = async (accountId: string) => {
    if (loading || switchingAccountId) return;

    setSwitchingAccountId(accountId);
    try {
      await switchAccount(accountId, 'app');
      showToast(t("common.success"), "success");
    } catch (error) {
      console.error("[Accounts] Switch failed:", error);
      showToast(`${t("common.error")}: ${error}`, "error");
    } finally {
      // Add a small delay for smoother UX
      setTimeout(() => {
        setSwitchingAccountId(null);
      }, 500);
    }
  };

  const handleExport = async (ids: string[]) => {
    try {
      const path = await save({ defaultPath: `antigravity-accounts-${format(new Date(), 'yyyy-MM-dd_HH-mm-ss')}.json`, filters: [{ name: 'JSON', extensions: ['json'] }], title: t('accounts.export_account') });
      if (!path) return;
      const count = await saveAccountExport(ids, path);
      showToast(t('accounts.export_success', { count }), 'success');
    } catch (error) { showToast(t('accounts.export_failed', { error: String(error) }), 'error'); }
  };

  const handleRefresh = async (accountId: string) => {
    setRefreshingIds((prev) => {
      const next = new Set(prev);
      next.add(accountId);
      return next;
    });
    try {
      await refreshQuota(accountId);
      showToast(t("common.success"), "success");
    } catch (error) {
      showToast(`${t("common.error")}: ${error}`, "error");
    } finally {
      setRefreshingIds((prev) => {
        const next = new Set(prev);
        next.delete(accountId);
        return next;
      });
    }
  };

  const handleBatchDelete = () => {
    if (selectedIds.size === 0) return;
    setIsBatchDelete(true);
  };

  const handleDelete = (accountId: string) => {
    const account = accounts.find((item) => item.id === accountId);
    if (account) setAccountToDelete(account);
  };

  const handleEditLabel = (accountId: string) => {
    const account = accounts.find((item) => item.id === accountId);
    if (!account) return;
    setAccountToEditLabel(account);
    setLabelDraft(account.custom_label || '');
  };

  const cancelEditLabel = () => {
    if (isSavingLabel) return;
    setAccountToEditLabel(null);
    setLabelDraft('');
  };

  const executeUpdateLabel = async () => {
    if (!accountToEditLabel || isSavingLabel) return;
    setIsSavingLabel(true);
    try {
      await updateAccountLabel(accountToEditLabel.id, labelDraft.trim());
      showToast(t('accounts.remark_updated', '备注已更新'), 'success');
      setAccountToEditLabel(null);
      setLabelDraft('');
    } catch (error) {
      console.error('[Accounts] Update remark failed:', error);
      showToast(`${t('common.error')}: ${error}`, 'error');
    } finally {
      setIsSavingLabel(false);
    }
  };

  const executeDelete = async () => {
    if (!accountToDelete) return;
    const accountId = accountToDelete.id;
    setAccountToDelete(null);
    try {
      await deleteAccounts([accountId]);
      setSelectedIds((prev) => {
        const next = new Set(prev);
        next.delete(accountId);
        return next;
      });
      showToast(t("common.success"), "success");
    } catch (error) {
      console.error("[Accounts] Delete failed:", error);
      showToast(`${t("common.error")}: ${error}`, "error");
    }
  };

  const executeBatchDelete = async () => {
    setIsBatchDelete(false);
    try {
      const ids = Array.from(selectedIds);
      console.log("[Accounts] Batch deleting:", ids);
      await deleteAccounts(ids);
      setSelectedIds(new Set());
      console.log("[Accounts] Batch delete success");
      showToast(t("common.success"), "success");
    } catch (error) {
      console.error("[Accounts] Batch delete failed:", error);
      showToast(`${t("common.error")}: ${error}`, "error");
    }
  };

  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isRefreshConfirmOpen, setIsRefreshConfirmOpen] = useState(false);

  const handleRefreshClick = () => {
    setIsRefreshConfirmOpen(true);
  };

  const executeRefresh = async () => {
    setIsRefreshConfirmOpen(false);
    setIsRefreshing(true);
    try {
      const isBatch = selectedIds.size > 0;
      let successCount = 0;
      let failedCount = 0;
      const details: string[] = [];

      if (isBatch) {
        // 批量刷新选中
        const ids = Array.from(selectedIds);
        setRefreshingIds(new Set(ids));

        const results = await Promise.allSettled(
          ids.map((id) => refreshQuota(id)),
        );

        results.forEach((result, index) => {
          const id = ids[index];
          const email = accounts.find((a) => a.id === id)?.email || id;
          if (result.status === "fulfilled") {
            successCount++;
          } else {
            failedCount++;
            details.push(`${email}: ${result.reason}`);
          }
        });
      } else {
        // 刷新所有
        setRefreshingIds(new Set(accounts.map((a) => a.id)));
        const stats = await useAccountStore.getState().refreshAllQuotas();
        if (stats) {
          successCount = stats.success;
          failedCount = stats.failed;
          details.push(...stats.details);
        }
      }

      if (failedCount === 0) {
        showToast(
          t("accounts.refresh_selected", { count: successCount }),
          "success",
        );
      } else {
        showToast(
          `${t("common.success")}: ${successCount}, ${t("common.error")}: ${failedCount}`,
          "warning",
        );
        // You might want to show details in a different way, but for toast, keep it simple or use a "view details" action if supported.
        // For now, simpler toast is better than a huge alert.
        if (details.length > 0) {
          console.warn("Refresh failures:", details);
        }
      }
    } catch (error) {
      showToast(`${t("common.error")}: ${error}`, "error");
    } finally {
      setIsRefreshing(false);
      setRefreshingIds(new Set());
    }
  };

  return (
    <div className="h-full flex flex-col p-5 gap-4 max-w-7xl mx-auto w-full">
      {/* 顶部工具栏:搜索、过滤和操作按钮 */}
      <div className="flex-none flex flex-wrap items-center gap-2">
        {/* 搜索框 - 响应式:大屏显示输入框,小屏显示图标 */}
        <div className="hidden lg:block flex-none w-48 relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400" />
          <input
            type="text"
            placeholder={t('accounts.search_placeholder')}
            className="w-full pl-9 pr-4 py-2 bg-white dark:bg-base-100 text-sm text-gray-900 dark:text-base-content border border-gray-200 dark:border-base-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent placeholder:text-gray-400 dark:placeholder:text-gray-500"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>

        {/* 搜索按钮 - 小屏显示 */}
        <div className="lg:hidden relative">
          {!isSearchExpanded ? (
            <button
              onClick={() => {
                setIsSearchExpanded(true);
                setTimeout(() => searchInputRef.current?.focus(), 100);
              }}
              className="p-2 bg-gray-100 dark:bg-base-200 hover:bg-gray-200 dark:hover:bg-base-100 rounded-lg transition-colors"
              title={t('accounts.search_placeholder')}
            >
              <Search className="w-4 h-4 text-gray-600 dark:text-gray-300" />
            </button>
          ) : (
            <div className="absolute left-0 top-0 z-10 w-64 flex items-center gap-1">
              <div className="flex-1 relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400" />
                <input
                  ref={searchInputRef}
                  type="text"
                  placeholder={t('accounts.search_placeholder')}
                  className="w-full pl-9 pr-4 py-2 bg-white dark:bg-base-100 text-sm text-gray-900 dark:text-base-content border border-gray-200 dark:border-base-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent placeholder:text-gray-400 dark:placeholder:text-gray-500 shadow-lg"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onBlur={() => setIsSearchExpanded(false)}
                />
              </div>
            </div>
          )}
        </div>

        {/* 配额周期切换 (5H / 7天周配额) */}
        <div className="flex gap-1 bg-gray-100 dark:bg-base-200 p-1 rounded-lg shrink-0 items-center">
          <button
            className={cn(
              "px-2 py-1 text-xs font-semibold rounded-md transition-all flex items-center gap-1",
              quotaWindow === "5h"
                ? "bg-white dark:bg-base-100 text-blue-600 dark:text-blue-400 shadow-sm"
                : "text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-base-content",
            )}
            onClick={() => setQuotaWindow("5h")}
            title={t("accounts.quota_window_5h", "5小时滑动配额")}
          >
            <Clock className="w-3.5 h-3.5" />
            <span>5H</span>
          </button>
          <button
            className={cn(
              "px-2 py-1 text-xs font-semibold rounded-md transition-all flex items-center gap-1",
              quotaWindow === "weekly"
                ? "bg-white dark:bg-base-100 text-blue-600 dark:text-blue-400 shadow-sm"
                : "text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-base-content",
            )}
            onClick={() => setQuotaWindow("weekly")}
            title={t("accounts.quota_window_weekly", "7天周配额")}
          >
            <Calendar className="w-3.5 h-3.5" />
            <span>{t("accounts.quota_window_weekly_short", "周配额")}</span>
          </button>
        </div>

        {/* 视图切换按钮组 */}
        <div className="flex gap-1 bg-gray-100 dark:bg-base-200 p-1 rounded-lg shrink-0">
          <button
            className={cn(
              "p-1.5 rounded-md transition-all",
              viewMode === "list"
                ? "bg-white dark:bg-base-100 text-blue-600 dark:text-blue-400 shadow-sm"
                : "text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-base-content",
            )}
            onClick={() => setViewMode("list")}
            title={t("accounts.views.list")}
          >
            <List className="w-4 h-4" />
          </button>
          <button
            className={cn(
              "p-1.5 rounded-md transition-all",
              viewMode === "grid"
                ? "bg-white dark:bg-base-100 text-blue-600 dark:text-blue-400 shadow-sm"
                : "text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-base-content",
            )}
            onClick={() => setViewMode("grid")}
            title={t("accounts.views.grid")}
          >
            <LayoutGrid className="w-4 h-4" />
          </button>
        </div>

        {/* 过滤按钮组 - 图标化响应式 */}
        <div className="flex gap-0.5 bg-gray-100/80 dark:bg-base-200 p-1 rounded-xl border border-gray-200/50 dark:border-white/5 shrink-0">
          {/* 全部 */}
          <button
            className={cn(
              "px-2 md:px-3 py-1.5 rounded-lg text-[11px] font-semibold transition-all flex items-center gap-1 md:gap-1.5 whitespace-nowrap shrink-0",
              filter === 'all'
                ? "bg-white dark:bg-base-100 text-blue-600 dark:text-blue-400 shadow-sm ring-1 ring-black/5"
                : "text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-base-content hover:bg-white/40"
            )}
            onClick={() => setFilter('all')}
            title={`${t('accounts.all')} (${filterCounts.all})`}
          >
            <span className="hidden md:inline">{t('accounts.all')}</span>
            <span className={cn(
              "px-1.5 py-0.5 rounded-md text-[10px] font-bold transition-colors",
              filter === 'all'
                ? "bg-blue-100 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400"
                : "bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400"
            )}>
              {filterCounts.all}
            </span>
          </button>

          {/* PRO */}
          <button
            className={cn(
              "px-2 md:px-3 py-1.5 rounded-lg text-[11px] font-semibold transition-all flex items-center gap-1 md:gap-1.5 whitespace-nowrap shrink-0",
              filter === 'pro'
                ? "bg-white dark:bg-base-100 text-blue-600 dark:text-blue-400 shadow-sm ring-1 ring-black/5"
                : "text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-base-content hover:bg-white/40"
            )}
            onClick={() => setFilter('pro')}
            title={`${t('accounts.pro')} (${filterCounts.pro})`}
          >
            <span className="hidden md:inline">{t('accounts.pro')}</span>
            <span className={cn(
              "px-1.5 py-0.5 rounded-md text-[10px] font-bold transition-colors",
              filter === 'pro'
                ? "bg-blue-100 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400"
                : "bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400"
            )}>
              {filterCounts.pro}
            </span>
          </button>

          {/* ULTRA */}
          <button
            className={cn(
              "flex px-2 lg:px-3 py-1.5 rounded-lg text-[11px] font-semibold transition-all items-center gap-1 lg:gap-1.5 whitespace-nowrap shrink-0",
              filter === 'ultra'
                ? "bg-white dark:bg-base-100 text-blue-600 dark:text-blue-400 shadow-sm ring-1 ring-black/5"
                : "text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-base-content hover:bg-white/40"
            )}
            onClick={() => setFilter('ultra')}
            title={`${t('accounts.ultra')} (${filterCounts.ultra})`}
          >
            <span className="hidden md:inline">{t('accounts.ultra')}</span>
            <span className={cn(
              "px-1.5 py-0.5 rounded-md text-[10px] font-bold transition-colors",
              filter === 'ultra'
                ? "bg-blue-100 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400"
                : "bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400"
            )}>
              {filterCounts.ultra}
            </span>
          </button>

          {/* FREE */}
          <button
            className={cn(
              "flex px-2 lg:px-3 py-1.5 rounded-lg text-[11px] font-semibold transition-all items-center gap-1 lg:gap-1.5 whitespace-nowrap shrink-0",
              filter === 'free'
                ? "bg-white dark:bg-base-100 text-blue-600 dark:text-blue-400 shadow-sm ring-1 ring-black/5"
                : "text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-base-content hover:bg-white/40"
            )}
            onClick={() => setFilter('free')}
            title={`${t('accounts.free')} (${filterCounts.free})`}
          >
            <span className="hidden md:inline">{t('accounts.free')}</span>
            <span className={cn(
              "px-1.5 py-0.5 rounded-md text-[10px] font-bold transition-colors",
              filter === 'free'
                ? "bg-blue-100 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400"
                : "bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400"
            )}>
              {filterCounts.free}
            </span>
          </button>
          {(filterCounts.unknown > 0 || filter === 'unknown') && (
            <button
              className={cn(
                "px-2 md:px-3 py-1.5 rounded-lg text-[11px] font-semibold transition-all flex items-center gap-1 md:gap-1.5 whitespace-nowrap shrink-0",
                filter === 'unknown'
                  ? "bg-white dark:bg-base-100 text-blue-600 dark:text-blue-400 shadow-sm ring-1 ring-black/5"
                  : "text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-base-content hover:bg-white/40"
              )}
              onClick={() => setFilter('unknown')}
              title={`${t('accounts.unknown')} (${filterCounts.unknown})`}
            >
              <span className="hidden md:inline">{t('accounts.unknown')}</span>
              <span className={cn(
                "px-1.5 py-0.5 rounded-md text-[10px] font-bold transition-colors",
                filter === 'unknown'
                  ? "bg-blue-100 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400"
                  : "bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400"
              )}>
                {filterCounts.unknown}
              </span>
            </button>
          )}
        </div>

        <div className="flex-1 min-w-[8px]"></div>

        {/* 操作按钮组 */}
        <div className="flex items-center gap-1.5 shrink-0">
          <AddAccountDialog onAdd={handleAddAccount} showText={false} />

          {selectedIds.size > 0 && (
            <>
              <button
                className="px-2.5 py-2 bg-red-500 text-white text-xs font-medium rounded-lg hover:bg-red-600 transition-colors flex items-center gap-1.5 shadow-sm"
                onClick={handleBatchDelete}
                title={t("accounts.delete_selected", {
                  count: selectedIds.size,
                })}
              >
                <Trash2 className="w-3.5 h-3.5" />
                <span className="hidden xl:inline">
                  {t("accounts.delete_selected", { count: selectedIds.size })}
                </span>
              </button>
            </>
          )}

          <button
            className={`px-2.5 py-2 bg-blue-500 text-white text-xs font-medium rounded-lg hover:bg-blue-600 transition-colors flex items-center gap-1.5 shadow-sm whitespace-nowrap shrink-0 ${isRefreshing ? "opacity-70 cursor-not-allowed" : ""}`}
            onClick={handleRefreshClick}
            disabled={isRefreshing}
            title={
              selectedIds.size > 0
                ? t("accounts.refresh_selected", { count: selectedIds.size })
                : t("accounts.refresh_all")
            }
          >
            <RefreshCw
              className={`w-3.5 h-3.5 ${isRefreshing ? "animate-spin" : ""}`}
            />
            <span className="hidden xl:inline">
              {isRefreshing
                ? t("common.loading")
                : selectedIds.size > 0
                  ? t("accounts.refresh_selected", { count: selectedIds.size })
                  : t("accounts.refresh_all")}
            </span>
          </button>

          {selectedIds.size > 0 && (
            <button
              className="px-2.5 py-2 bg-emerald-500 text-white text-xs font-medium rounded-lg hover:bg-emerald-600 transition-colors flex items-center gap-1.5 shadow-sm whitespace-nowrap shrink-0"
              onClick={() => handleExport([...selectedIds])}
              aria-label={t('accounts.export_selected', { count: selectedIds.size })}
              title={t('accounts.export_selected', { count: selectedIds.size })}
            >
              <Download className="w-3.5 h-3.5" />
            </button>
          )}

        </div>
      </div>

      {/* 账号列表内容区域 */}
      <div className="flex-1 min-h-0 relative" ref={containerRef}>
        {viewMode === "list" ? (
          <div className="h-full bg-white dark:bg-base-100 rounded-2xl shadow-sm border border-gray-100 dark:border-base-200 flex flex-col overflow-hidden">
            <div className="flex-1 overflow-y-auto">
              <AccountTable
                accounts={paginatedAccounts}
                selectedIds={selectedIds}
                refreshingIds={refreshingIds}
                onToggleSelect={handleToggleSelect}
                onToggleAll={handleToggleAll}
                currentAccountId={currentAccount?.id || null}
                switchingAccountId={switchingAccountId}
                onSwitch={handleSwitch}
                onRefresh={handleRefresh}
                onEditLabel={handleEditLabel}
                onDelete={handleDelete}
                onExport={id => handleExport([id])}
                onReorder={reorderAccounts}
                quotaWindow={quotaWindow}
              />
            </div>
          </div>
        ) : (
          <div className="h-full overflow-y-auto">
            <AccountGrid
              accounts={paginatedAccounts}
              selectedIds={selectedIds}
              refreshingIds={refreshingIds}
              onToggleSelect={handleToggleSelect}
              currentAccountId={currentAccount?.id || null}
              switchingAccountId={switchingAccountId}
              onSwitch={handleSwitch}
              onRefresh={handleRefresh}
              onEditLabel={handleEditLabel}
              onDelete={handleDelete}
              onExport={id => handleExport([id])}
              onReorder={reorderAccounts}
              quotaWindow={quotaWindow}
            />
          </div>
        )}
      </div>

      {/* 极简分页 - 无边框浮动样式 */}
      {filteredAccounts.length > 0 && (
        <div className="flex-none">
          <Pagination
            currentPage={currentPage}
            totalPages={Math.ceil(filteredAccounts.length / ITEMS_PER_PAGE)}
            onPageChange={handlePageChange}
            totalItems={filteredAccounts.length}
            itemsPerPage={ITEMS_PER_PAGE}
            onPageSizeChange={(newSize) => {
              setLocalPageSize(newSize);
              setCurrentPage(1); // 重置到第一页
            }}
            pageSizeOptions={[10, 20, 50, 100]}
          />
        </div>
      )}

      <ModalDialog
        isOpen={isBatchDelete}
        title={t("accounts.dialog.batch_delete_title")}
        message={t("accounts.dialog.batch_delete_msg", { count: selectedIds.size })}
        type="confirm"
        confirmText={t("common.delete")}
        isDestructive={true}
        onConfirm={executeBatchDelete}
        onCancel={() => setIsBatchDelete(false)}
      />

      <ModalDialog
        isOpen={accountToDelete !== null}
        title={t("accounts.dialog.delete_title")}
        message={accountToDelete
          ? `${t("accounts.dialog.delete_msg")}\n\n${accountToDelete.email}`
          : undefined}
        type="confirm"
        confirmText={t("common.delete")}
        isDestructive={true}
        onConfirm={executeDelete}
        onCancel={() => setAccountToDelete(null)}
      />

      <ModalDialog
        isOpen={isRefreshConfirmOpen}
        title={
          selectedIds.size > 0
            ? t("accounts.dialog.batch_refresh_title")
            : t("accounts.dialog.refresh_title")
        }
        message={
          selectedIds.size > 0
            ? t("accounts.dialog.batch_refresh_msg", {
              count: selectedIds.size,
            })
            : t("accounts.dialog.refresh_msg")
        }
        type="confirm"
        confirmText={t("common.refresh")}
        isDestructive={false}
        onConfirm={executeRefresh}
        onCancel={() => setIsRefreshConfirmOpen(false)}
      />

      <ModalDialog
        isOpen={accountToEditLabel !== null}
        title={t('accounts.edit_remark', '编辑备注')}
        type="confirm"
        confirmText={isSavingLabel ? t('common.loading') : t('common.save')}
        onConfirm={executeUpdateLabel}
        onCancel={cancelEditLabel}
      >
        <div className="space-y-2">
          <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
            {t('accounts.remark', '备注')}
          </label>
          <input
            autoFocus
            type="text"
            maxLength={15}
            value={labelDraft}
            onChange={(event) => setLabelDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault();
                void executeUpdateLabel();
              }
            }}
            placeholder={t('accounts.remark_placeholder', '输入备注（最多15个字符）')}
            className="w-full px-3 py-2.5 bg-white dark:bg-base-200 text-gray-900 dark:text-base-content border border-gray-200 dark:border-base-300 rounded-xl focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          <div className="text-right text-xs text-gray-400 dark:text-gray-500">
            {Array.from(labelDraft).length}/15
          </div>
        </div>
      </ModalDialog>



    </div>
  );
}

export default Accounts;
