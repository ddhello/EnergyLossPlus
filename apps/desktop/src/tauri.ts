import { invoke } from "@tauri-apps/api/core";
import type { AppBootstrap, AppSnapshot, DiaryMonth, ExerciseEntry, FoodEntry, GoalRecommendation, ProfileInput, Session, WeightEntry } from "./types";

const isTauri = "__TAURI_INTERNALS__" in window;
const isBrowserDev = import.meta.env.DEV && !isTauri;
const API_BASE = (
  import.meta.env.VITE_API_BASE_URL || "https://x38dzo14cd.execute-api.ap-northeast-1.amazonaws.com"
).replace(/\/+$/, "");
const webSessionKey = "energylossplus.webSession";

export function isBrowserDevelopment(): boolean {
  return isBrowserDev;
}

const demoSnapshot: AppSnapshot = {
  session: {
    token: "browser-dev-demo",
    userId: "demo",
    nickname: "Browser Dev",
    deviceName: "Vite",
    expiresAt: "2099-12-31T23:59:59Z"
  },
  profile: {
    sex: "male",
    ageYears: 34,
    heightCm: 178,
    weightKg: 82,
    activityLevel: "moderate",
    goalKind: "lose"
  },
  recommendation: {
    bmr: 1768,
    tdee: 2740,
    dailyCalorieTarget: 2340,
    macros: { proteinG: 164, carbsG: 321, fatG: 73 }
  },
  dailyCalorieTarget: 2340,
  foods: [
    {
      id: "demo-food-1",
      userId: "demo",
      date: today(),
      meal: "Breakfast",
      name: "Oats, eggs, blueberries",
      calories: 520,
      proteinG: 28,
      carbsG: 62,
      fatG: 18
    },
    {
      id: "demo-food-2",
      userId: "demo",
      date: today(),
      meal: "Lunch",
      name: "Chicken rice bowl",
      calories: 760,
      proteinG: 56,
      carbsG: 82,
      fatG: 21
    }
  ],
  exercises: [
    {
      id: "demo-exercise-1",
      userId: "demo",
      date: today(),
      name: "Brisk walk",
      caloriesBurned: 260,
      durationMinutes: 45
    }
  ],
  weights: [
    { id: "demo-weight-1", userId: "demo", date: "2026-06-05", weightKg: 82.6 },
    { id: "demo-weight-2", userId: "demo", date: "2026-06-06", weightKg: 82.3 },
    { id: "demo-weight-3", userId: "demo", date: "2026-06-07", weightKg: 82.1 },
    { id: "demo-weight-4", userId: "demo", date: today(), weightKg: 81.9 }
  ],
  syncStatus: "cached"
};

export async function loadSnapshot(month = today().slice(0, 7)): Promise<AppSnapshot> {
  if (isBrowserDev) return { ...demoSnapshot, ...diaryForMonth(demoSnapshot, month) };
  if (!isTauri) {
    const session = readWebSession();
    return session ? loadWebMonth(session.token, month) : { ...demoSnapshot, session: undefined };
  }
  return invoke<AppSnapshot>("load_cached_snapshot", { month });
}

export async function calculateGoal(profile: ProfileInput): Promise<GoalRecommendation> {
  if (!isTauri) {
    return demoSnapshot.recommendation!;
  }
  return invoke<GoalRecommendation>("calculate_goal", { profile });
}

export async function syncSnapshot(token: string, month = today().slice(0, 7)): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return { ...demoSnapshot, ...diaryForMonth(demoSnapshot, month), syncStatus: "online" };
  }
  if (!isTauri) return loadWebMonth(token, month);
  return invoke<AppSnapshot>("sync_snapshot", { token, month });
}

export async function loadDiaryMonth(token: string, month: string): Promise<DiaryMonth> {
  if (isBrowserDev) return diaryForMonth(demoSnapshot, month);
  if (!isTauri) return webApi<DiaryMonth>(`/v2/diary?month=${month}`, token);
  return invoke<DiaryMonth>("load_diary_month", { token, month });
}

export async function saveSession(session: Session): Promise<void> {
  if (isBrowserDev) {
    return;
  }
  if (!isTauri) {
    window.localStorage.setItem(webSessionKey, JSON.stringify(session));
    return;
  }
  await invoke("save_session", { session });
}

export async function clearSession(): Promise<void> {
  if (!isTauri) {
    window.localStorage.removeItem(webSessionKey);
    return;
  }
  await invoke("clear_session");
}

export async function updateGoal(token: string, profile: ProfileInput): Promise<AppBootstrap> {
  if (isBrowserDev) {
    return { ...demoSnapshot, session: demoSnapshot.session!, profile, syncStatus: "online" };
  }
  if (!isTauri) return webApi<AppBootstrap>("/v2/goal", token, "PUT", profile);
  return invoke<AppBootstrap>("update_goal", { token, profile });
}

export async function updateDailyTarget(token: string, dailyCalorieTarget: number): Promise<AppBootstrap> {
  if (isBrowserDev) {
    return { ...demoSnapshot, session: demoSnapshot.session!, dailyCalorieTarget, syncStatus: "online" };
  }
  if (!isTauri) return webApi<AppBootstrap>("/v2/daily-target", token, "PUT", { dailyCalorieTarget });
  return invoke<AppBootstrap>("update_daily_target", { token, dailyCalorieTarget });
}

export async function createFood(token: string, entry: CreateFoodInput): Promise<FoodEntry> {
  if (isBrowserDev) {
    return { ...entry, id: crypto.randomUUID(), userId: "demo" };
  }
  if (!isTauri) return webApi<FoodEntry>("/v2/foods", token, "POST", entry);
  return invoke<FoodEntry>("create_food", { token, entry });
}

export async function updateFood(token: string, id: string, originalDate: string, entry: CreateFoodInput): Promise<FoodEntry> {
  if (isBrowserDev) return { ...entry, id, userId: "demo" };
  if (!isTauri) return webApi<FoodEntry>(`/v2/foods/${originalDate}/${id}`, token, "PUT", entry);
  return invoke<FoodEntry>("update_food", { token, id, originalDate, entry });
}

export async function deleteFood(token: string, id: string, date: string): Promise<void> {
  if (isBrowserDev) return;
  if (!isTauri) return webApi<void>(`/v2/foods/${date}/${id}`, token, "DELETE");
  return invoke<void>("delete_food", { token, id, date });
}

export async function createExercise(token: string, entry: CreateExerciseInput): Promise<ExerciseEntry> {
  if (isBrowserDev) return { ...entry, id: crypto.randomUUID(), userId: "demo" };
  if (!isTauri) return webApi<ExerciseEntry>("/v2/exercises", token, "POST", entry);
  return invoke<ExerciseEntry>("create_exercise", { token, entry });
}

export async function updateExercise(token: string, id: string, originalDate: string, entry: CreateExerciseInput): Promise<ExerciseEntry> {
  if (isBrowserDev) return { ...entry, id, userId: "demo" };
  if (!isTauri) return webApi<ExerciseEntry>(`/v2/exercises/${originalDate}/${id}`, token, "PUT", entry);
  return invoke<ExerciseEntry>("update_exercise", { token, id, originalDate, entry });
}

export async function deleteExercise(token: string, id: string, date: string): Promise<void> {
  if (isBrowserDev) return;
  if (!isTauri) return webApi<void>(`/v2/exercises/${date}/${id}`, token, "DELETE");
  return invoke<void>("delete_exercise", { token, id, date });
}

export async function createWeight(token: string, entry: CreateWeightInput): Promise<WeightEntry> {
  if (isBrowserDev) return { ...entry, id: crypto.randomUUID(), userId: "demo" };
  if (!isTauri) return webApi<WeightEntry>("/v2/weights", token, "POST", entry);
  return invoke<WeightEntry>("create_weight", { token, entry });
}

export async function updateWeight(token: string, id: string, originalDate: string, entry: CreateWeightInput): Promise<WeightEntry> {
  if (isBrowserDev) return { ...entry, id, userId: "demo" };
  if (!isTauri) return webApi<WeightEntry>(`/v2/weights/${originalDate}/${id}`, token, "PUT", entry);
  return invoke<WeightEntry>("update_weight", { token, id, originalDate, entry });
}

export async function deleteWeight(token: string, id: string, date: string): Promise<void> {
  if (isBrowserDev) return;
  if (!isTauri) return webApi<void>(`/v2/weights/${date}/${id}`, token, "DELETE");
  return invoke<void>("delete_weight", { token, id, date });
}

function today(): string {
  return new Date().toISOString().slice(0, 10);
}

function readWebSession(): Session | undefined {
  try {
    return JSON.parse(window.localStorage.getItem(webSessionKey) || "") as Session;
  } catch {
    return undefined;
  }
}

async function loadWebMonth(token: string, month: string): Promise<AppSnapshot> {
  const [bootstrap, diary] = await Promise.all([
    webApi<AppBootstrap>("/v2/bootstrap", token),
    webApi<DiaryMonth>(`/v2/diary?month=${month}`, token)
  ]);
  return { ...bootstrap, ...diary };
}

function diaryForMonth(snapshot: AppSnapshot, month: string): DiaryMonth {
  return {
    foods: snapshot.foods.filter((entry) => entry.date.startsWith(month)),
    exercises: snapshot.exercises.filter((entry) => entry.date.startsWith(month)),
    weights: snapshot.weights.filter((entry) => entry.date.startsWith(month))
  };
}

async function webApi<T>(path: string, token: string, method = "GET", body?: unknown): Promise<T> {
  const response = await fetch(`${API_BASE}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      ...(body === undefined ? {} : { "Content-Type": "application/json" })
    },
    body: body === undefined ? undefined : JSON.stringify(body)
  });
  if (!response.ok) throw new Error(await response.text());
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export interface CreateFoodInput {
  date: string;
  meal: string;
  name: string;
  calories: number;
  proteinG: number;
  carbsG: number;
  fatG: number;
  note?: string;
}

export interface CreateExerciseInput {
  date: string;
  name: string;
  caloriesBurned: number;
  durationMinutes: number;
  note?: string;
}

export interface CreateWeightInput {
  date: string;
  weightKg: number;
  note?: string;
}
