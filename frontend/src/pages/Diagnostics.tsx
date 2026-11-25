import { useState } from 'react';
import { Activity, Bug, CircleCheck, CircleX, RefreshCw, ShieldCheck } from 'lucide-react';
import Toggle from '../components/Toggle';
import { api, type AppConfig, type EndpointHealth, type LogEvent } from '../lib/tauri';

interface Props {
  config: AppConfig;
  events: LogEvent[];
}

export default function Diagnostics({ config, events }: Props) {
  const [health, setHealth] = useState<EndpointHealth[]>([]);
  const [probe, setProbe] = useState('');
  const [loading, setLoading] = useState(false);

  const refresh = async () => {
    setLoading(true);
    try {
      const [h, p] = await Promise.all([api.endpointHealth(), api.testApi(config)]);
      setHealth(h);
      setProbe(p);
    } catch (error) {
      setProbe(String(error));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="grid-bg min-h-full space-y-5 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold text-white">Диагностика</h1>
          <p className="mt-1 text-xs text-slate-500">Состояние API, анонимности и фоновых процессов</p>
        </div>
        <button className="btn-ghost" onClick={() => void refresh()} disabled={loading}>
          <RefreshCw size={15} className={loading ? 'animate-spin' : ''} /> Обновить
        </button>
      </div>

      <div className="grid gap-5 xl:grid-cols-2">
        <section className="panel p-5">
          <h2 className="panel-title"><Activity size={16} /> Проверка API</h2>
          <div className={`rounded-lg border px-3 py-2 text-xs ${probe ? 'border-emerald-500/25 bg-emerald-500/10 text-emerald-300' : 'border-slate-700 text-slate-500'}`}>
            {probe || 'Нажмите «Обновить», чтобы проверить доступность бесплатных API.'}
          </div>
          <div className="mt-4 space-y-2">
            {health.length === 0 && <div className="text-[11px] text-slate-600">Нет данных о endpoints.</div>}
            {health.map((h) => (
              <div key={`${h.coin}-${h.name}`} className="flex items-center justify-between gap-3 rounded-lg border border-slate-700/50 bg-slate-950/40 px-3 py-2 text-[11px]">
                <div className="flex items-center gap-2">
                  {h.quarantined_for_seconds > 0 ? <CircleX size={14} className="text-rose-400" /> : <CircleCheck size={14} className="text-emerald-400" />}
                  <span className="text-slate-300">{h.coin} · {h.name}</span>
                </div>
                <div className="text-right text-slate-500">
                  <div>успехов: {h.ok} · ошибок: {h.fail}</div>
                  {h.quarantined_for_seconds > 0 && <div className="text-rose-400">карантин {h.quarantined_for_seconds}с</div>}
                </div>
              </div>
            ))}
          </div>
        </section>

        <section className="panel p-5">
          <h2 className="panel-title"><ShieldCheck size={16} /> Состояние анонимности</h2>
          <div className="grid grid-cols-2 gap-2 text-[11px]">
            {[
              ['Мастер', config.full_anonymity_mode], ['Сеть', config.anonymity_mode],
              ['Tor', config.use_tor], ['Прокси', !!config.proxy_url],
              ['User-Agent', config.random_user_agent], ['Заголовки', config.random_headers],
              ['Шифрование', config.encryption_enabled], ['В памяти', config.memory_only_mode],
              ['Килл-свитч', config.kill_switch], ['Блок direct', config.block_direct_connection],
              ['Stealth', config.stealth_mode], ['Маска окна', config.stealth_window_title],
            ].map(([label, on]) => (
              <div key={String(label)} className={`flex items-center justify-between rounded-lg border px-3 py-2 ${
                on ? 'border-emerald-500/20 bg-emerald-500/5 text-emerald-300' : 'border-slate-700/50 text-slate-500'
              }`}>
                <span>{label}</span><Toggle checked={Boolean(on)} onChange={() => undefined} disabled />
              </div>
            ))}
          </div>
        </section>

        <section className="panel p-5 xl:col-span-2">
          <h2 className="panel-title"><Bug size={16} /> Лог приложения</h2>
          <div className="max-h-64 space-y-1 overflow-y-auto font-mono text-[11px]">
            {events.length === 0 && <div className="text-slate-600">Событий пока нет.</div>}
            {events.map((event, i) => (
              <div key={i} className={event.kind === 'error' ? 'text-rose-300' : event.kind === 'hit' ? 'text-emerald-300' : 'text-slate-400'}>
                <span className="text-slate-600">{event.time}</span> {event.text}
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
