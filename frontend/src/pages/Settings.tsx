import { useState } from 'react';
import type { ReactNode } from 'react';
import {
  Activity, Ban, Bug, CheckCircle2, Clipboard, Cpu, Download, EyeOff, FileText,
  Fingerprint, Gauge, KeyRound, LockKeyhole, RefreshCw, Save, Search, Send,
  Settings2, ShieldCheck, Timer, Upload, Wallet, XCircle,
} from 'lucide-react';
import Toggle from '../components/Toggle';
import {
  api, COINS, MODES,
  type AppConfig, type EndpointHealth,
} from '../lib/tauri';

interface Props {
  config: AppConfig;
  setConfig: (config: AppConfig) => void;
}

function Panel({ title, icon, children }: { title: string; icon: ReactNode; children: ReactNode }) {
  return (
    <section className="panel p-5">
      <h2 className="panel-title">{icon}{title}</h2>
      <div className="space-y-4">{children}</div>
    </section>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="min-w-[45%] flex-1">
        <div className="text-sm text-slate-200">{label}</div>
        {hint && <div className="mt-0.5 text-[11px] leading-snug text-slate-500">{hint}</div>}
      </div>
      <div className="flex items-center gap-3">{children}</div>
    </div>
  );
}

function NumberField({ label, value, min, max, suffix, onChange }: {
  label: string; value: number; min: number; max: number; suffix?: string;
  onChange: (value: number) => void;
}) {
  return (
    <label className="block">
      <span className="field-label">{label}</span>
      <div className="flex items-center gap-2">
        <input className="input" type="number" min={min} max={max} value={value}
          onChange={(e) => onChange(Math.min(max, Math.max(min, Number(e.target.value) || min)))} />
        {suffix && <span className="text-[11px] text-slate-500">{suffix}</span>}
      </div>
    </label>
  );
}

function TextField({ label, value, placeholder, type, onChange }: {
  label: string; value: string; placeholder?: string; type?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="block">
      <span className="field-label">{label}</span>
      <input className="input mono" type={type ?? 'text'} value={value} placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)} />
    </label>
  );
}

function SelectField({ label, value, options, onChange }: {
  label: string; value: string; options: readonly string[]; onChange: (value: string) => void;
}) {
  return (
    <label className="block">
      <span className="field-label">{label}</span>
      <select className="input" value={value} onChange={(e) => onChange(e.target.value)}>
        {options.map((option) => <option key={option} value={option}>{option}</option>)}
      </select>
    </label>
  );
}

const LANGUAGES = ['english', 'spanish', 'french', 'italian', 'japanese', 'korean', 'chinese_simplified', 'chinese_traditional'] as const;
const ADDRESS_TYPES = ['auto', 'legacy', 'segwit', 'native', 'taproot'] as const;

export default function Settings({ config, setConfig }: Props) {
  const [status, setStatus] = useState('');
  const [warnings, setWarnings] = useState<string[]>([]);
  const [apiStatus, setApiStatus] = useState('');
  const [health, setHealth] = useState<EndpointHealth[]>([]);
  const [preview, setPreview] = useState<[string, string, string][]>([]);
  const [exported, setExported] = useState('');
  const [importText, setImportText] = useState('');

  const patch = (partial: Partial<AppConfig>) => setConfig({ ...config, ...partial });

  const save = async () => {
    try {
      const messages = await api.saveConfig(config);
      setWarnings(messages);
      setStatus('Настройки сохранены');
    } catch (error) {
      setStatus(`Ошибка сохранения: ${String(error)}`);
    }
  };

  const toggleMaster = (enabled: boolean) => {
    const next = { ...config, full_anonymity_mode: enabled };
    if (enabled) {
      next.anonymity_mode = true;
      next.random_user_agent = true;
      next.random_headers = true;
      next.stealth_mode = true;
      next.stealth_window_title = true;
      next.encryption_enabled = true;
      next.memory_only_mode = true;
      next.kill_switch = true;
      if (!next.encryption_password) next.encryption_password = `HappinessAnon_${Math.random().toString(36).slice(2, 14)}`;
    }
    setConfig(next);
  };

  const runTest = async () => {
    setApiStatus('Проверяю API…');
    try { setApiStatus(await api.testApi(config)); }
    catch (error) { setApiStatus(String(error)); }
  };

  const refreshHealth = async () => {
    try { setHealth(await api.endpointHealth()); }
    catch (error) { setStatus(String(error)); }
  };

  const runPreview = async () => {
    try { setPreview(await api.preview(config, 3)); }
    catch (error) { setStatus(String(error)); }
  };

  const doExport = async () => {
    try {
      const text = await api.exportConfig();
      setExported(text);
      await navigator.clipboard?.writeText(text).catch(() => undefined);
      setStatus('Конфиг скопирован в буфер обмена');
    } catch (error) { setStatus(String(error)); }
  };

  const doImport = async () => {
    try {
      const parsed = await api.importConfig(importText);
      setConfig(parsed);
      setStatus('Конфиг импортирован — нажмите «Сохранить»');
    } catch (error) { setStatus(String(error)); }
  };

  return (
    <div className="grid-bg min-h-full space-y-5 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold text-white">Настройки</h1>
          <p className="mt-1 text-xs text-slate-500">Всё автоматизировано. Сохраните настройки — и нажмите СТАРТ на дашборде.</p>
        </div>
        <button className="btn-primary" onClick={save}><Save size={16} /> Сохранить</button>
      </div>

      {status && <div className="rounded-lg border border-emerald-500/25 bg-emerald-500/10 px-4 py-2 text-xs text-emerald-300">{status}</div>}
      {warnings.length > 0 && (
        <div className="space-y-1 rounded-lg border border-amber-500/25 bg-amber-500/5 px-4 py-3 text-[11px] text-amber-300">
          {warnings.map((warning) => <div key={warning}>⚠ {warning}</div>)}
        </div>
      )}

      <div className="grid gap-5 xl:grid-cols-2">
        <Panel title="Степень нагрузки" icon={<Cpu size={17} />}>
          <NumberField label="Параллельных воркеров" value={config.workers} min={1} max={32} suffix="потоков"
            onChange={(v) => patch({ workers: v })} />
          <NumberField label="Размер батча" value={config.batch_size} min={1} max={500} suffix="адресов за запрос"
            onChange={(v) => patch({ batch_size: v })} />
          <NumberField label="Задержка между батчами" value={config.delay_ms} min={0} max={600000} suffix="мс"
            onChange={(v) => patch({ delay_ms: v })} />
          <p className="text-[11px] text-slate-500">Больше воркеров = больше шансов, но выше риск блокировки API. Увеличьте задержку вместе с числом потоков.</p>
        </Panel>

        <Panel title="Что ищем" icon={<Search size={17} />}>
          <SelectField label="Монета" value={config.coin} options={['ALL', ...COINS]} onChange={(v) => patch({ coin: v })} />
          <SelectField label="Режим поиска" value={config.scan_mode} options={MODES} onChange={(v) => patch({ scan_mode: v })} />
          <div className="grid grid-cols-2 gap-3">
            <SelectField label="Тип адреса" value={config.address_type} options={ADDRESS_TYPES} onChange={(v) => patch({ address_type: v })} />
            <SelectField label="Язык BIP39" value={config.bip39_language} options={LANGUAGES} onChange={(v) => patch({ bip39_language: v })} />
          </div>
          <TextField label="Ванити-префикс (для режима VANITY_PATTERN)" value={config.vanity_prefix} placeholder="напр. 1abc"
            onChange={(v) => patch({ vanity_prefix: v })} />
          <TextField label="Файл словаря (одна фраза на строку)" value={config.dictionary_file} placeholder="C:\\words.txt"
            onChange={(v) => patch({ dictionary_file: v })} />
          <Row label="Сохранять пустые адреса" hint="Записываются в results/empty.jsonl (если не включён режим «только в памяти»)">
            <Toggle checked={config.save_empty} onChange={(v) => patch({ save_empty: v })} />
          </Row>
          <button className="btn-ghost w-full" onClick={runPreview}><EyeOff size={15} /> Сгенерировать пример</button>
          {preview.length > 0 && (
            <div className="space-y-1 rounded-lg bg-slate-950/60 p-3 font-mono text-[10px] text-slate-400">
              {preview.map(([coin, address, phrase], i) => (
                <div key={i}><span className="text-emerald-400">{coin}</span> {address} <span className="text-slate-600">← {phrase.slice(0, 46)}…</span></div>
              ))}
            </div>
          )}
        </Panel>

        <Panel title="Кэш проверенных адресов" icon={<FileText size={17} />}>
          <Row label="Использовать кэш" hint="Один адрес не проверяется повторно в течение TTL — экономия запросов и защита от бана">
            <Toggle checked={config.address_cache_enabled} onChange={(v) => patch({ address_cache_enabled: v })} />
          </Row>
          <NumberField label="TTL записи" value={config.address_cache_ttl_minutes} min={1} max={100800} suffix="минут"
            onChange={(v) => patch({ address_cache_ttl_minutes: v })} />
        </Panel>

        <Panel title="Полная анонимность — всё одной кнопкой" icon={<ShieldCheck size={17} />}>
          <Row label="Мастер анонимности (рекомендуется)" hint="Все защиты ниже включаются автоматически. Пароль шифрования генерируется сам.">
            <Toggle checked={config.full_anonymity_mode} onChange={toggleMaster} />
          </Row>
          <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/5 p-3 text-[11px] leading-relaxed text-emerald-200/75">
            Tor/SOCKS5 · случайный User-Agent · случайные HTTP-заголовки · ротация прокси · джиттер ·
            stealth · маска окна · AES-256-GCM · только в памяти · килл-свитч · блок прямого HTTP.
          </div>
        </Panel>

        <Panel title="Сеть и маскировка" icon={<Fingerprint size={17} />}>
          <Row label="Анонимный режим"><Toggle checked={config.anonymity_mode} onChange={(v) => patch({ anonymity_mode: v })} /></Row>
          <Row label="Tor / SOCKS5" hint="Весь трафик через локальный Tor (127.0.0.1:9050)">
            <Toggle checked={config.use_tor} onChange={(v) => patch({ use_tor: v })} />
          </Row>
          <TextField label="Адрес Tor SOCKS5" value={config.tor_proxy} onChange={(v) => patch({ tor_proxy: v })} />
          <TextField label="Дополнительный прокси (http://user:pass@host:port)" value={config.proxy_url} onChange={(v) => patch({ proxy_url: v })} />
          <Row label="Ротация прокси"><Toggle checked={config.proxy_rotation} onChange={(v) => patch({ proxy_rotation: v })} /></Row>
          <Row label="Случайный User-Agent"><Toggle checked={config.random_user_agent} onChange={(v) => patch({ random_user_agent: v })} /></Row>
          <Row label="Случайные HTTP-заголовки"><Toggle checked={config.random_headers} onChange={(v) => patch({ random_headers: v })} /></Row>
          <Row label="Stealth-режим"><Toggle checked={config.stealth_mode} onChange={(v) => patch({ stealth_mode: v })} /></Row>
          <Row label="Маскировка заголовка окна" hint="Окно отображается как «System Host Process»">
            <Toggle checked={config.stealth_window_title} onChange={(v) => {
              patch({ stealth_window_title: v });
              void api.setStealthTitle(v);
            }} />
          </Row>
        </Panel>

        <Panel title="Шифрование и килл-свитч" icon={<LockKeyhole size={17} />}>
          <Row label="Шифровать результаты (AES-256-GCM)" hint="PBKDF2-SHA256, 100 000 итераций">
            <Toggle checked={config.encryption_enabled} onChange={(v) => patch({ encryption_enabled: v })} />
          </Row>
          <TextField label="Пароль шифрования" type="password" value={config.encryption_password}
            placeholder="генерируется автоматически" onChange={(v) => patch({ encryption_password: v })} />
          <Row label="Только в памяти" hint="Ничего не пишется на диск — даже зашифрованные результаты">
            <Toggle checked={config.memory_only_mode} onChange={(v) => patch({ memory_only_mode: v })} />
          </Row>
          <Row label="Килл-свитч" hint="Останавливает сканирование при угрозе утечки">
            <Toggle checked={config.kill_switch} onChange={(v) => patch({ kill_switch: v })} />
          </Row>
          <NumberField label="Порог ошибок API" value={config.kill_switch_threshold} min={1} max={1000} suffix="ошибок"
            onChange={(v) => patch({ kill_switch_threshold: v })} />
          <Row label="Блокировать прямой HTTP" hint="Без прокси запросы к API запрещены — реальный IP не раскрывается">
            <Toggle checked={config.block_direct_connection} onChange={(v) => patch({ block_direct_connection: v })} />
          </Row>
        </Panel>

        <Panel title="Уведомления" icon={<Send size={17} />}>
          <Row label="Telegram"><Toggle checked={config.telegram_enabled} onChange={(v) => patch({ telegram_enabled: v })} /></Row>
          <TextField label="Токен бота" type="password" value={config.telegram_token} onChange={(v) => patch({ telegram_token: v })} />
          <TextField label="Chat ID" value={config.telegram_chat_id} onChange={(v) => patch({ telegram_chat_id: v })} />
          <Row label="Discord webhook"><Toggle checked={config.discord_enabled} onChange={(v) => patch({ discord_enabled: v })} /></Row>
          <TextField label="URL webhook" type="password" value={config.discord_webhook} onChange={(v) => patch({ discord_webhook: v })} />
        </Panel>

        <Panel title="Система" icon={<Settings2 size={17} />}>
          <Row label="Автозапуск с Windows" hint="Запись в HKCU Run, запуск свёрнутым в трей">
            <Toggle checked={config.autostart} onChange={(v) => {
              patch({ autostart: v });
              void api.setAutostart(v).catch(() => undefined);
            }} />
          </Row>
          <Row label="Запускать свёрнутым"><Toggle checked={config.start_minimized} onChange={(v) => patch({ start_minimized: v })} /></Row>
          <div className="flex flex-wrap gap-2 pt-1">
            <button className="btn-ghost" onClick={() => void api.setWindowVisible(false)}><EyeOff size={15} /> Скрыть окно</button>
            <button className="btn-ghost" onClick={() => void api.setWindowVisible(true)}><Activity size={15} /> Показать окно</button>
          </div>
        </Panel>

        <Panel title="Экспорт и импорт настроек" icon={<Download size={17} />}>
          <div className="flex flex-wrap gap-2">
            <button className="btn-ghost" onClick={() => void doExport()}><Clipboard size={15} /> Экспорт в буфер</button>
            <button className="btn-ghost" onClick={() => void doImport()}><Upload size={15} /> Импорт JSON</button>
          </div>
          {exported && <textarea readOnly className="input mono h-28 w-full resize-none text-[10px]" value={exported} />}
          <textarea className="input mono h-28 w-full resize-none text-[10px]" placeholder="Вставьте JSON конфигурации…"
            value={importText} onChange={(e) => setImportText(e.target.value)} />
        </Panel>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3 pb-6">
        <button className="btn-primary" onClick={save}><Save size={16} /> Сохранить все настройки</button>
        <div className="flex items-center gap-2 text-[11px] text-slate-500">
          <KeyRound size={13} /> Пароль шифрования хранится локально и никуда не отправляется
        </div>
      </div>
    </div>
  );
}
