import { useRef, useState } from 'react';
import { DndContext, KeyboardSensor, PointerSensor, closestCenter, useSensor, useSensors } from '@dnd-kit/core';
import { SortableContext, arrayMove, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { GripVertical, LayoutDashboard } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import { request } from '../../utils/request';
import { DASHBOARD_CARD_IDS, dashboardCards, type DashboardCardId, type DashboardPreferences } from '../../types/config';

function CardRow({ id, selected, disabled, onToggle }: { id: DashboardCardId; selected: boolean; disabled: boolean; onToggle: () => void }) {
    const { t } = useTranslation();
    const label = t(`dashboard_settings.cards.${id}`);
    const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id, disabled: disabled || !selected });
    return <div ref={setNodeRef} data-card-option={id} style={{ transform: CSS.Transform.toString(transform), transition }} className={`flex items-center gap-3 rounded-lg border border-slate-200 p-3 text-sm dark:border-slate-700 ${isDragging ? 'relative z-10 bg-blue-50 shadow-sm dark:bg-slate-800' : ''}`}>
        <button type="button" {...attributes} {...listeners} disabled={disabled || !selected} aria-label={t('dashboard_settings.reorder', { card: label })} className="touch-none rounded p-1 text-slate-500 hover:bg-slate-100 focus-visible:ring-2 focus-visible:ring-blue-500 disabled:opacity-20 dark:hover:bg-slate-700"><GripVertical size={16} /></button>
        <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-3">
            <input type="checkbox" disabled={disabled} checked={selected} onChange={onToggle} className="h-4 w-4 shrink-0 accent-blue-600" />
            <span className="text-xs font-medium text-slate-700 dark:text-slate-200">{label}</span>
        </label>
    </div>;
}

export default function DashboardSettings() {
    const { t } = useTranslation();
    const config = useConfigStore(state => state.config);
    const [pending, setPending] = useState<DashboardCardId[] | null>(null);
    const [error, setError] = useState('');
    const lock = useRef(false);
    const selected = pending ?? dashboardCards(config?.dashboard?.cards);
    const ordered = [...selected, ...DASHBOARD_CARD_IDS.filter(id => !selected.includes(id))];
    const disabled = pending !== null || !config;
    const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }), useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }));
    const update = async (cards: DashboardCardId[]) => {
        if (lock.current || !config) return;
        lock.current = true; setPending(cards); setError('');
        try {
            const next = await request<DashboardPreferences>('set_dashboard_cards', { cards });
            useConfigStore.setState(state => ({ config: state.config ? { ...state.config, dashboard: next } : null }));
        } catch (error) {
            setError(t('dashboard_settings.save_failed', { error: String(error) }));
        } finally {
            lock.current = false; setPending(null);
        }
    };
    const label = (id: string | number) => t(`dashboard_settings.cards.${id}`);
    return <section aria-labelledby="dashboard-settings-title" className="rounded-2xl border border-gray-200/80 bg-white p-5 sm:p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/80">
        <div className="mb-5 flex items-start gap-3">
            <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-blue-50 text-blue-600 dark:bg-blue-400/10 dark:text-blue-300"><LayoutDashboard className="h-5 w-5" /></span>
            <div>
                <h3 id="dashboard-settings-title" className="text-base font-semibold text-gray-900 dark:text-gray-100">{t('dashboard_settings.title')}</h3>
                <p className="mt-0.5 text-xs leading-relaxed text-gray-500 dark:text-gray-400">{t('dashboard_settings.description')}</p>
            </div>
        </div>
        <p className="mb-3 text-xs text-gray-500 dark:text-gray-400" role="status">{t(pending !== null ? 'dashboard_settings.saving' : 'dashboard_settings.selected_count', { count: selected.length })}</p>
        <DndContext sensors={sensors} collisionDetection={closestCenter} accessibility={{ screenReaderInstructions: { draggable: t('dashboard_settings.drag_instructions') }, announcements: {
            onDragStart: ({ active }) => t('dashboard_settings.drag_start', { card: label(active.id) }),
            onDragOver: ({ over }) => over ? t('dashboard_settings.drag_over', { card: label(over.id) }) : '',
            onDragEnd: () => t('dashboard_settings.drag_end'),
            onDragCancel: () => t('dashboard_settings.drag_cancel'),
        } }} onDragEnd={({ active, over }) => {
            if (disabled || !over || active.id === over.id) return;
            const from = selected.indexOf(active.id as DashboardCardId), to = selected.indexOf(over.id as DashboardCardId);
            if (from >= 0 && to >= 0) void update(arrayMove(selected, from, to));
        }}>
            <SortableContext items={selected} strategy={verticalListSortingStrategy}>
                <div className="grid gap-2">{ordered.map(id => <CardRow key={id} id={id} selected={selected.includes(id)} disabled={disabled} onToggle={() => void update(selected.includes(id) ? selected.filter(card => card !== id) : [...selected, id])} />)}</div>
            </SortableContext>
        </DndContext>
        {error && <p role="alert" className="mt-3 text-xs text-red-600">{error}</p>}
    </section>;
}
