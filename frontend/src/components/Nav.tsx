import { Activity, Bug, Coins, Radio, Settings2, ShieldCheck, X } from 'lucide-react';

export type Page = 'dashboard' | 'settings' | 'hits' | 'diagnostics';

interface NavProps {
  page: Page;
  setPage: (page: Page) => void;
  running: boolean;
  isDemo: boolean;
}

const items: { id: Page; label: string; icon: typeof Activity }[] = [
  { id: 'dashboard', label: 'Дашборд', icon: Activity },
  { id: 'hits', label: 'Найденные', icon: Coins },
  { id: 'settings', label: 'Настройки', icon: Settings2 },
  { id: 'diagnostics', label: 'Диагностика', icon: Bug },
];

export default function Nav({ page, setPage, running, isDemo }: NavProps) {
  return (
    <aside className="flex w-[232px] shrink-0 flex-col border-r border-emerald-500/10 bg-[#0a141b]/80 px-4 py-6">
      <div className="mb-8 flex items-center gap-3 px-2">
        <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-emerald-500/15 text-emerald-300 ring-1 ring-emerald-400/30">
          <Radio size={19} />
        </div>
        <div>
          <div className="text-sm font-bold tracking-wide text-white">Happiness</div>
          <div className="text-[10px] uppercase tracking-[0.2em] text-emerald-400/70">scanner v0.6.3</div>
        </div>
      </div>

      <nav className="flex flex-col gap-1.5">
        {items.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => setPage(id)}
            className={`flex items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm transition ${
              page === id
                ? 'bg-emerald-500/15 text-emerald-300 ring-1 ring-emerald-500/20'
                : 'text-slate-400 hover:bg-white/5 hover:text-slate-200'
            }`}
          >
            <Icon size={17} />
            {label}
          </button>
        ))}
      </nav>

      <div className="mt-auto space-y-3 px-1">
        <div className={`chip w-full justify-center ${running ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300' : 'border-slate-700 text-slate-400'}`}>
          <span className={`h-2 w-2 rounded-full ${running ? 'animate-pulse2 bg-emerald-400' : 'bg-slate-500'}`} />
          {running ? 'ПОИСК ИДЁТ' : 'ОЖИДАНИЕ'}
        </div>
        <div className="chip w-full justify-center border-emerald-500/20 bg-emerald-500/5 text-emerald-200/80">
          <ShieldCheck size={13} /> FULL ANONYMITY
        </div>
        {isDemo && (
          <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-2 py-1.5 text-center text-[10px] text-amber-300/80">
            демо-режим браузера
          </div>
        )}
        <div className="flex items-center justify-center gap-1 pt-1 text-[10px] text-slate-600">
          <X size={10} /> трей · автостарт · шифрование
        </div>
      </div>
    </aside>
  );
}
