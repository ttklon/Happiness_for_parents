import { useCallback, useEffect, useRef, useState } from 'react';
import { Bell, Coins, LoaderCircle, Play, Square } from 'lucide-react';
import Nav, { type Page } from './components/Nav';
import Dashboard from './pages/Dashboard';
import Hits from './pages/Hits';
import Settings from './pages/Settings';
import Diagnostics from './pages/Diagnostics';
import {
  api, DEFAULT_CONFIG, EMPTY_STATS, formatCoinAmount, isTauri,
  type AppConfig, type HitRecord, type LogEvent, type ScanStats, type UpdateInfo,
} from './lib/tauri';

export default function App() {
  const [page, setPage] = useState<Page>('dashboard');
  const [config, setConfig] = useState<AppConfig>(DEFAULT_CONFIG);
  const [stats, setStats] = useState<ScanStats>(EMPTY_STATS);
  const [events, setEvents] = useState<LogEvent[]>([]);
  const [history, setHistory] = useState<number[]>([]);
  const [hits, setHits] = useState<HitRecord[]>([]);
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState<HitRecord | null>(null);
  const lastHit = useRef<string | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [s, e, h] = await Promise.all([api.getStats(), api.getEvents(), api.getHistory()]);
      setStats(s);
      setEvents(e);
      setHistory(h);
    } catch {
      // The backend may still be starting up — retry on the next tick.
    }
  }, []);

  const refreshHits = useCallback(async () => {
    try { setHits(await api.getHits()); }
    catch { /* ignore */ }
  }, []);

  useEffect(() => {
    let alive = true;
    (async () => {
      try { const cfg = await api.loadConfig(); if (alive) setConfig(cfg); }
      catch { /* defaults are fine */ }
    })();
    void refresh();
    void refreshHits();
    const timer = window.setInterval(() => { void refresh(); void refreshHits(); }, 900);
    return () => { alive = false; window.clearInterval(timer); };
  }, [refresh, refreshHits]);

  // Check GitHub for a new release once a minute — no user action required.
  useEffect(() => {
    if (!config.update_check_enabled) return;
    let alive = true;
    const check = async () => {
      try {
        const info = await api.checkUpdates();
        if (alive && info.available) setUpdateInfo(info);
      } catch { /* offline / rate limited — try again next minute */ }
    };
    void check();
    const timer = window.setInterval(() => void check(), 60_000);
    return () => { alive = false; window.clearInterval(timer); };
  }, [config.update_check_enabled]);

  useEffect(() => {
    const first = hits[0];
    if (first && first.id !== lastHit.current) {
      lastHit.current = first.id;
      setToast(first);
      const timer = window.setTimeout(() => setToast(null), 6000);
      return () => window.clearTimeout(timer);
    }
  }, [hits]);

  const start = async () => {
    setBusy(true);
    try { await api.saveConfig(config); await api.start(config); }
    finally { setBusy(false); void refresh(); }
  };

  const stop = async () => {
    setBusy(true);
    try { await api.stop(); }
    finally { setBusy(false); void refresh(); }
  };

  const clearHits = async () => {
    await api.clearHits();
    setHits([]);
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-ink text-slate-200">
      <Nav page={page} setPage={setPage} running={stats.running} isDemo={!isTauri()} />

      <main className="relative flex-1 overflow-y-auto">
        <header className="sticky top-0 z-20 flex items-center justify-between gap-3 border-b border-emerald-500/10 bg-[#08121a]/85 px-6 py-3 backdrop-blur">
          <div className="flex items-center gap-2 text-xs text-slate-500">
            <span className="h-2 w-2 rounded-full bg-emerald-500 shadow-[0_0_12px_#22d39a]" />
            Happiness Scanner <span className="text-slate-700">/</span> <span className="text-slate-300">{page}</span>
          </div>
          <div className="flex items-center gap-3">
            <span className="hidden items-center gap-1 text-[11px] text-slate-500 sm:flex">
              <Coins size={13} /> {stats.total_checked.toLocaleString('ru-RU')} проверок
            </span>
            <button className="btn-ghost !px-3 !py-1.5 !text-xs" onClick={() => setPage('hits')}>
              <Bell size={14} /> {stats.hits}
            </button>
            {stats.running ? (
              <button className="btn-danger !px-3 !py-1.5 !text-xs" onClick={() => void stop()} disabled={busy}>
                <Square size={13} /> Стоп
              </button>
            ) : (
              <button className="btn-primary !px-3 !py-1.5 !text-xs" onClick={() => void start()} disabled={busy}>
                {busy ? <LoaderCircle size={13} className="animate-spin" /> : <Play size={13} fill="currentColor" />} Старт
              </button>
            )}
          </div>
        </header>

        {page === 'dashboard' && (
          <Dashboard stats={stats} config={config} events={events} history={history} hits={hits}
            onStart={() => void start()} onStop={() => void stop()}
            onHide={() => void api.setWindowVisible(false)} />
        )}
        {page === 'settings' && <Settings config={config} setConfig={setConfig} />}
        {page === 'hits' && <Hits hits={hits} onClear={() => void clearHits()} />}
        {page === 'diagnostics' && <Diagnostics config={config} events={events} />}
      </main>

      {toast && (
        <div className="fixed bottom-6 right-6 z-50 w-[360px] rounded-xl border border-emerald-500/30 bg-[#0d1c22]/95 p-4 shadow-2xl shadow-black/50 backdrop-blur">
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2 text-sm font-semibold text-emerald-300">🎯 Найден кошелёк!</div>
            <button className="text-slate-500 hover:text-slate-200" onClick={() => setToast(null)}>×</button>
          </div>
          <div className="mt-2 text-xs text-slate-300"><span className="text-emerald-400">{toast.coin}</span> · {formatCoinAmount(toast.coin, toast.balance)}</div>
          <div className="mono mt-1 truncate text-[11px] text-slate-500">{toast.address}</div>
        </div>
      )}

      {updateInfo?.available && (
        <div className="fixed bottom-6 left-1/2 z-50 w-[420px] -translate-x-1/2 rounded-xl border border-emerald-400/40 bg-[#0d1c22]/95 p-4 shadow-2xl shadow-black/50 backdrop-blur">
          <div className="flex items-center justify-between gap-3">
            <div className="text-sm font-semibold text-emerald-300">🆕 Доступно обновление {updateInfo.version}</div>
            <button className="text-slate-500 hover:text-slate-200" onClick={() => setUpdateInfo(null)}>×</button>
          </div>
          <div className="mono mt-1 text-[11px] text-slate-500">{updateInfo.asset_name} · SHA-256 проверяется перед установкой</div>
          <div className="mt-3 flex gap-2">
            <button className="btn-primary !py-1.5 !text-xs" onClick={() => void api.installUpdate(updateInfo)}>Обновить сейчас</button>
            <button className="btn-ghost !py-1.5 !text-xs" onClick={() => setPage('settings')}>Настройки</button>
          </div>
        </div>
      )}
    </div>
  );
}
