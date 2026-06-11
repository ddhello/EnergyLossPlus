import {
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  Flame,
  Home,
  KeyRound,
  LogOut,
  Plus,
  RefreshCw,
  Trash2,
  Utensils
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { isExternalAuthAvailable, listenForExternalAuth, startExternalAuth } from "./external-auth";
import { errorMessage, isPasskeyAvailable, loginWithPasskey, passkeyUnavailableReason, registerWithPasskey } from "./passkey";
import { clearSession, createFood, deleteFood, isBrowserDevelopment, loadSnapshot, saveSession, syncSnapshot, updateDailyTarget } from "./tauri";
import type { AppSnapshot, FoodEntry, ProfileInput } from "./types";

type View = "today" | "history";
type CalorieInputMode = "direct" | "package";
type EnergyUnit = "kcal" | "kJ";

const initialProfile: ProfileInput = {
  sex: "male",
  ageYears: 34,
  heightCm: 178,
  weightKg: 82,
  activityLevel: "moderate",
  goalKind: "lose"
};

const dailyTargetKey = "energylossplus.dailyTarget";
const meals = ["早餐", "午餐", "晚餐", "加餐"];

export function App() {
  const [snapshot, setSnapshot] = useState<AppSnapshot>({
    profile: initialProfile,
    foods: [],
    exercises: [],
    weights: [],
    syncStatus: "cached"
  });
  const [view, setView] = useState<View>("today");
  const [nickname, setNickname] = useState("new-user");
  const [deviceName, setDeviceName] = useState("Phone");
  const [meal, setMeal] = useState("午餐");
  const [foodName, setFoodName] = useState("");
  const [caloriesInput, setCaloriesInput] = useState("400");
  const [calorieInputMode, setCalorieInputMode] = useState<CalorieInputMode>("direct");
  const [energyUnit, setEnergyUnit] = useState<EnergyUnit>("kcal");
  const [labelEnergyInput, setLabelEnergyInput] = useState("");
  const [labelGramsInput, setLabelGramsInput] = useState("100");
  const [packageGramsInput, setPackageGramsInput] = useState("");
  const [dailyTarget, setDailyTarget] = useState(readDailyTarget);
  const [dailyTargetInput, setDailyTargetInput] = useState(() => String(readDailyTarget()));
  const [selectedDate, setSelectedDate] = useState(todayString);
  const [calendarMonth, setCalendarMonth] = useState(() => todayString().slice(0, 7));
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [dockHidden, setDockHidden] = useState(false);

  useEffect(() => {
    loadSnapshot()
      .then((loaded) => {
        setSnapshot(loaded);
        applySyncedDailyTarget(loaded);
      })
      .catch((error) => console.error(error));
  }, []);

  useEffect(() => {
    window.localStorage?.setItem(dailyTargetKey, String(dailyTarget));
  }, [dailyTarget]);

  useEffect(() => {
    if (!isExternalAuthAvailable()) return;
    let unlisten: (() => void) | undefined;
    void listenForExternalAuth(
      (session) => void completeLogin(session),
      (error) => setMessage(errorMessage(error))
    ).then((stop) => {
      unlisten = stop;
    }).catch((error) => setMessage(errorMessage(error)));
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    let lastScrollY = window.scrollY;
    let ticking = false;
    const updateDock = () => {
      const scrollY = Math.max(0, window.scrollY);
      const delta = scrollY - lastScrollY;
      if (scrollY < 24) {
        setDockHidden(false);
      } else if (delta > 10) {
        setDockHidden(true);
      } else if (delta < -10) {
        setDockHidden(false);
      }
      lastScrollY = scrollY;
      ticking = false;
    };
    const handleScroll = () => {
      if (ticking) return;
      ticking = true;
      window.requestAnimationFrame(updateDock);
    };
    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  useEffect(() => {
    setDockHidden(false);
  }, [view]);

  const today = todayString();
  const todayFoods = useMemo(() => foodsForDate(snapshot.foods, today), [snapshot.foods, today]);
  const selectedFoods = useMemo(() => foodsForDate(snapshot.foods, selectedDate), [snapshot.foods, selectedDate]);
  const caloriesByDate = useMemo(() => snapshot.foods.reduce<Record<string, number>>((totals, entry) => {
    totals[entry.date] = (totals[entry.date] ?? 0) + entry.calories;
    return totals;
  }, {}), [snapshot.foods]);
  const consumed = caloriesByDate[today] ?? 0;
  const remaining = dailyTarget - consumed;
  const progress = Math.min(100, Math.round((consumed / Math.max(dailyTarget, 1)) * 100));
  const mealIndex = Math.max(0, meals.indexOf(meal));
  const calories = calorieInputMode === "direct"
    ? positiveNumber(caloriesInput)
    : calculatePackageCalories(energyUnit, labelEnergyInput, labelGramsInput, packageGramsInput);
  const passkeyAvailable = isPasskeyAvailable();
  const passkeyNotice = passkeyUnavailableReason();
  const externalAuthAvailable = isExternalAuthAvailable();

  async function completeLogin(session: import("./types").Session) {
    await saveSession(session);
    const synced = await syncSnapshot(session.token).catch(() => null);
    if (synced) {
      setSnapshot(synced);
      applySyncedDailyTarget(synced);
    } else {
      setSnapshot((current) => ({ ...current, session, syncStatus: "online" }));
    }
    setMessage("");
  }

  async function handleAuth(mode: "register" | "login" | "recover") {
    setBusy(true);
    setMessage("");
    try {
      if (externalAuthAvailable) {
        await startExternalAuth(mode, nickname.trim(), deviceName.trim() || "iPhone");
        setMessage("已在 Safari 打开登录页，完成 Passkey 后将自动返回 App。");
        return;
      }
      if (mode === "recover") {
        throw new Error("账号恢复必须在外部 HTTPS Passkey 页面中完成。");
      }
      const session = mode === "register"
        ? await registerWithPasskey(nickname.trim(), deviceName.trim() || "Phone")
        : await loginWithPasskey(nickname.trim());
      await completeLogin(session);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSync() {
    if (!snapshot.session) return;
    setBusy(true);
    setMessage("");
    try {
      const synced = await syncSnapshot(snapshot.session.token);
      setSnapshot(synced);
      applySyncedDailyTarget(synced);
    } catch (error) {
      setSnapshot((current) => ({ ...current, syncStatus: "offline" }));
      setMessage(error instanceof Error ? error.message : "同步失败，当前显示本地缓存。");
    } finally {
      setBusy(false);
    }
  }

  async function handleDailyTargetBlur() {
    const target = dailyTargetNumber(dailyTargetInput);
    if (target === null) {
      setDailyTargetInput(String(dailyTarget));
      return;
    }
    if (!snapshot.session || target === snapshot.dailyCalorieTarget) return;
    setBusy(true);
    setMessage("");
    try {
      const synced = await updateDailyTarget(snapshot.session.token, target);
      setSnapshot(synced);
      applySyncedDailyTarget(synced);
    } catch (error) {
      setSnapshot((current) => ({ ...current, syncStatus: "offline" }));
      setMessage(error instanceof Error ? error.message : "每日目标同步失败，当前保留本地设置。");
    } finally {
      setBusy(false);
    }
  }

  function applySyncedDailyTarget(loaded: AppSnapshot) {
    const target = loaded.dailyCalorieTarget
      ?? (window.localStorage?.getItem(dailyTargetKey) ? undefined : loaded.recommendation?.dailyCalorieTarget);
    if (!target) return;
    setDailyTarget(target);
    setDailyTargetInput(String(target));
  }

  async function handleAddMeal() {
    if (!snapshot.session || !foodName.trim() || calories === null) return;
    setBusy(true);
    setMessage("");
    try {
      setSnapshot(await createFood(snapshot.session.token, {
        date: today,
        meal,
        name: foodName.trim(),
        calories,
        proteinG: 0,
        carbsG: 0,
        fatG: 0
      }));
      setFoodName("");
      if (calorieInputMode === "direct") {
        setCaloriesInput("");
      } else {
        setLabelEnergyInput("");
        setPackageGramsInput("");
      }
    } catch (error) {
      setSnapshot((current) => ({ ...current, syncStatus: "offline" }));
      setMessage(error instanceof Error ? error.message : "保存失败，云端确认前不会修改记录。");
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteMeal(entry: FoodEntry) {
    if (!snapshot.session) return;
    setBusy(true);
    setMessage("");
    try {
      setSnapshot(await deleteFood(snapshot.session.token, entry.id));
    } catch (error) {
      setSnapshot((current) => ({ ...current, syncStatus: "offline" }));
      setMessage(error instanceof Error ? error.message : "删除失败。");
    } finally {
      setBusy(false);
    }
  }

  async function handleLogout() {
    await clearSession();
    if (isBrowserDevelopment()) {
      setSnapshot(await loadSnapshot());
      return;
    }
    setSnapshot((current) => ({ ...current, session: undefined, syncStatus: "cached" }));
  }

  if (!snapshot.session) {
    return (
      <main className="phone-shell auth-screen">
        <div className="ambient ambient-one" />
        <div className="ambient ambient-two" />
        <section className="auth-card glass-card">
          <div className="app-badge"><KeyRound size={25} /></div>
          <div className="auth-heading">
            <span className="eyebrow">轻松记录 · 清晰掌握</span>
            <h1>EnergyLossPlus</h1>
            <p>用 Passkey 安全登录，开始记录你的每日能量。</p>
          </div>
          <label>昵称<input value={nickname} onChange={(event) => setNickname(event.target.value)} /></label>
          <label>设备名称<input value={deviceName} onChange={(event) => setDeviceName(event.target.value)} /></label>
          {passkeyNotice && !externalAuthAvailable && <div className="notice">{passkeyNotice}</div>}
          {message && <div className="notice">{message}</div>}
          <button className="primary-button" disabled={busy || (!externalAuthAvailable && (!nickname.trim() || !passkeyAvailable))} onClick={() => handleAuth("login")}>登录</button>
          <button className="secondary-button" disabled={busy || (!externalAuthAvailable && (!nickname.trim() || !passkeyAvailable))} onClick={() => handleAuth("register")}>创建 Passkey</button>
          <button className="secondary-button" disabled={busy || !externalAuthAvailable || !nickname.trim()} onClick={() => handleAuth("recover")}>恢复原账号</button>
        </section>
      </main>
    );
  }

  return (
    <main className="phone-shell">
      <div className="ambient ambient-one" />
      <div className="ambient ambient-two" />
      <section className="phone-app">
        <header className="mobile-header">
          <div>
            <span className="status-pill"><i />{syncLabel(snapshot.syncStatus)}</span>
            <h1>{view === "today" ? "今天" : "热量日历"}</h1>
          </div>
          <div className="header-actions">
            <button className="icon-button" aria-label="同步" title="同步" disabled={busy} onClick={handleSync}><RefreshCw size={18} /></button>
            <button className="icon-button" aria-label="退出" title="退出" onClick={handleLogout}><LogOut size={18} /></button>
          </div>
        </header>

        <div className={`tab-content tab-content-${view}`} key={view}>
        {view === "today" ? (
          <>
            <section className={`remaining-card glass-card ${remaining < 0 ? "over" : ""}`}>
              <div className="remaining-top">
                <div>
                  <span>今日剩余</span>
                  <div className="calorie-total"><strong>{remaining}</strong><em>kcal</em></div>
                </div>
                <div className="flame-badge"><Flame size={23} /></div>
              </div>
              <div className="progress-track"><i style={{ width: `${progress}%` }} /></div>
              <div className="progress-meta"><span>已摄入 {consumed}</span><span>{progress}%</span><span>目标 {dailyTarget}</span></div>
            </section>

            <section className="target-row glass-card">
              <label><span>每日目标</span><input type="number" min="500" max="6000" step="50" value={dailyTargetInput} onChange={(event) => {
                const value = event.target.value;
                setDailyTargetInput(value);
                const target = dailyTargetNumber(value);
                if (target !== null) setDailyTarget(target);
              }} onBlur={handleDailyTargetBlur} /></label>
              <span className="target-unit">kcal</span>
            </section>

            <section className="add-card glass-card">
              <div className="section-title"><div><span className="eyebrow">快速添加</span><h2>记一餐</h2></div></div>
              <div className="segmented" style={{ "--segment-index": mealIndex } as React.CSSProperties}>
                <span className="segment-slider" aria-hidden="true" />
                {meals.map((item) => <button key={item} className={meal === item ? "active" : ""} onClick={() => setMeal(item)}>{item}</button>)}
              </div>
              <label>吃了什么<input placeholder="例如：牛肉饭" value={foodName} onChange={(event) => setFoodName(event.target.value)} /></label>
              <div className="input-mode-switch" aria-label="热量输入方式" style={{ "--switch-index": calorieInputMode === "direct" ? 0 : 1 } as React.CSSProperties}>
                <span className="switch-slider" aria-hidden="true" />
                <button className={calorieInputMode === "direct" ? "active" : ""} onClick={() => setCalorieInputMode("direct")}>直接热量</button>
                <button className={calorieInputMode === "package" ? "active" : ""} onClick={() => setCalorieInputMode("package")}>包装换算</button>
              </div>
              {calorieInputMode === "direct" ? (
                <label className="calorie-input-panel direct" key="direct">热量<input type="number" min="1" step="10" value={caloriesInput} onChange={(event) => setCaloriesInput(event.target.value)} /></label>
              ) : (
                <div className="package-calculator calorie-input-panel package" key="package">
                  <div className="package-fields">
                    <label>营养表能量<input aria-label="营养表能量" type="number" min="0" step="1" value={labelEnergyInput} onChange={(event) => setLabelEnergyInput(event.target.value)} /></label>
                    <div className="unit-switch" aria-label="能量单位" style={{ "--switch-index": energyUnit === "kcal" ? 0 : 1 } as React.CSSProperties}>
                      <span className="switch-slider" aria-hidden="true" />
                      <button className={energyUnit === "kcal" ? "active" : ""} onClick={() => setEnergyUnit("kcal")}>kcal</button>
                      <button className={energyUnit === "kJ" ? "active" : ""} onClick={() => setEnergyUnit("kJ")}>kJ</button>
                    </div>
                    <label>对应克数<input aria-label="营养表对应克数" type="number" min="0" step="1" value={labelGramsInput} onChange={(event) => setLabelGramsInput(event.target.value)} /></label>
                    <label>一包克数<input aria-label="一包克数" type="number" min="0" step="1" value={packageGramsInput} onChange={(event) => setPackageGramsInput(event.target.value)} /></label>
                  </div>
                  <div className="calculation-result">
                    <span>整包热量</span>
                    <strong>{calories ?? "—"} <small>kcal</small></strong>
                  </div>
                </div>
              )}
              <button className="primary-button add-button" disabled={busy || !foodName.trim() || calories === null} onClick={handleAddMeal}><Plus size={18} /> 添加记录</button>
            </section>

            <MealLog title="今日餐单" entries={todayFoods} busy={busy} emptyText="还没有记录，先记下第一餐。" onDelete={handleDeleteMeal} />
          </>
        ) : (
          <>
            <Calendar month={calendarMonth} selectedDate={selectedDate} caloriesByDate={caloriesByDate} onMonthChange={setCalendarMonth} onSelectDate={setSelectedDate} />
            <section className="history-summary glass-card">
              <span>{formatFullDate(selectedDate)}</span>
              <strong>{caloriesByDate[selectedDate] ?? 0} <small>kcal</small></strong>
              <em>剩余 {dailyTarget - (caloriesByDate[selectedDate] ?? 0)} kcal</em>
            </section>
            <MealLog title="当日摄入日志" entries={selectedFoods} busy={busy} emptyText="这一天没有摄入记录。" onDelete={handleDeleteMeal} />
          </>
        )}
        </div>

        {message && <div className="notice">{message}</div>}
        <nav className={`bottom-nav ${dockHidden ? "hidden" : ""}`} aria-label="主导航">
          <button className={view === "today" ? "active" : ""} onClick={() => setView("today")}><Home size={20} /><span>今天</span></button>
          <button className={view === "history" ? "active" : ""} onClick={() => setView("history")}><CalendarDays size={20} /><span>历史</span></button>
        </nav>
      </section>
    </main>
  );
}

function Calendar({ month, selectedDate, caloriesByDate, onMonthChange, onSelectDate }: {
  month: string;
  selectedDate: string;
  caloriesByDate: Record<string, number>;
  onMonthChange: (month: string) => void;
  onSelectDate: (date: string) => void;
}) {
  return (
    <section className="calendar-card glass-card">
      <div className="calendar-header">
        <button className="icon-button" title="上个月" onClick={() => onMonthChange(shiftMonth(month, -1))}><ChevronLeft size={18} /></button>
        <strong>{formatMonth(month)}</strong>
        <button className="icon-button" title="下个月" onClick={() => onMonthChange(shiftMonth(month, 1))}><ChevronRight size={18} /></button>
      </div>
      <div className="weekday-row">{["一", "二", "三", "四", "五", "六", "日"].map((day) => <span key={day}>{day}</span>)}</div>
      <div className="calendar-grid">
        {calendarCells(month).map((date, index) => date ? (
          <button key={date} className={`${date === selectedDate ? "selected" : ""} ${date === todayString() ? "today" : ""}`} onClick={() => onSelectDate(date)}>
            <strong>{Number(date.slice(-2))}</strong><span>{caloriesByDate[date] ? `${caloriesByDate[date]}` : ""}</span>
          </button>
        ) : <span className="calendar-spacer" key={`spacer-${index}`} />)}
      </div>
    </section>
  );
}

function MealLog({ title, entries, busy, emptyText, onDelete }: {
  title: string;
  entries: FoodEntry[];
  busy: boolean;
  emptyText: string;
  onDelete: (entry: FoodEntry) => void;
}) {
  return (
    <section className="meal-list glass-card">
      <div className="section-title"><h2>{title}</h2><span>{entries.length} 餐</span></div>
      {entries.length === 0 ? (
        <div className="empty-state"><Utensils size={22} /><p>{emptyText}</p></div>
      ) : entries.map((entry) => (
        <article className="meal-row" key={entry.id}>
          <div className="meal-icon"><Utensils size={17} /></div>
          <div className="meal-content"><span>{entry.meal}</span><strong>{entry.name}</strong></div>
          <em>{entry.calories} <small>kcal</small></em>
          <button className="delete-button" title="删除" disabled={busy} onClick={() => onDelete(entry)}><Trash2 size={17} /></button>
        </article>
      ))}
    </section>
  );
}

function foodsForDate(foods: FoodEntry[], date: string): FoodEntry[] {
  return foods.filter((entry) => entry.date === date).sort((a, b) => a.id.localeCompare(b.id));
}

function calendarCells(month: string): Array<string | null> {
  const [year, monthNumber] = month.split("-").map(Number);
  const firstDay = new Date(year, monthNumber - 1, 1);
  const daysInMonth = new Date(year, monthNumber, 0).getDate();
  const mondayOffset = (firstDay.getDay() + 6) % 7;
  const cells: Array<string | null> = Array(mondayOffset).fill(null);
  for (let day = 1; day <= daysInMonth; day += 1) cells.push(`${month}-${String(day).padStart(2, "0")}`);
  return cells;
}

function shiftMonth(month: string, amount: number): string {
  const [year, monthNumber] = month.split("-").map(Number);
  const shifted = new Date(year, monthNumber - 1 + amount, 1);
  return `${shifted.getFullYear()}-${String(shifted.getMonth() + 1).padStart(2, "0")}`;
}

function formatMonth(month: string): string {
  const [year, monthNumber] = month.split("-");
  return `${year} 年 ${Number(monthNumber)} 月`;
}

function formatFullDate(date: string): string {
  const [year, month, day] = date.split("-");
  return `${year} 年 ${Number(month)} 月 ${Number(day)} 日`;
}

function syncLabel(status: AppSnapshot["syncStatus"]): string {
  return status === "online" ? "已连接云端" : status === "offline" ? "离线缓存" : "本地缓存";
}

function readDailyTarget(): number {
  const saved = Number(window.localStorage?.getItem(dailyTargetKey));
  return Number.isFinite(saved) && saved > 0 ? saved : 2200;
}

function positiveNumber(value: string): number | null {
  if (!value.trim()) return null;
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : null;
}

function dailyTargetNumber(value: string): number | null {
  const target = positiveNumber(value);
  return target !== null && target >= 500 && target <= 6000 ? target : null;
}

function calculatePackageCalories(unit: EnergyUnit, energyInput: string, basisGramsInput: string, packageGramsInput: string): number | null {
  const energy = positiveNumber(energyInput);
  const basisGrams = positiveNumber(basisGramsInput);
  const packageGrams = positiveNumber(packageGramsInput);
  if (energy === null || basisGrams === null || packageGrams === null) return null;
  const kcal = unit === "kJ" ? energy / 4.184 : energy;
  return Math.round((kcal / basisGrams) * packageGrams);
}

function todayString(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
}
