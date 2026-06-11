import { invoke } from "@tauri-apps/api/core";
import type { AppSnapshot, GoalRecommendation, ProfileInput, Session } from "./types";

const isTauri = "__TAURI_INTERNALS__" in window;
const isBrowserDev = import.meta.env.DEV && !isTauri;
const API_BASE = (
  import.meta.env.VITE_API_BASE_URL || "https://api.energylossplus.invalid"
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

export async function loadSnapshot(): Promise<AppSnapshot> {
  if (isBrowserDev) return demoSnapshot;
  if (!isTauri) {
    const session = readWebSession();
    return session ? webApi<AppSnapshot>("/snapshot", session.token) : { ...demoSnapshot, session: undefined };
  }
  return invoke<AppSnapshot>("load_cached_snapshot");
}

export async function calculateGoal(profile: ProfileInput): Promise<GoalRecommendation> {
  if (!isTauri) {
    return demoSnapshot.recommendation!;
  }
  return invoke<GoalRecommendation>("calculate_goal", { profile });
}

export async function syncSnapshot(token: string): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return { ...demoSnapshot, syncStatus: "online" };
  }
  if (!isTauri) return webApi<AppSnapshot>("/snapshot", token);
  return invoke<AppSnapshot>("sync_snapshot", { token });
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

export async function updateGoal(token: string, profile: ProfileInput): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return { ...demoSnapshot, profile, syncStatus: "online" };
  }
  if (!isTauri) return webApi<AppSnapshot>("/goal", token, "PUT", profile);
  return invoke<AppSnapshot>("update_goal", { token, profile });
}

export async function updateDailyTarget(token: string, dailyCalorieTarget: number): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return { ...demoSnapshot, dailyCalorieTarget, syncStatus: "online" };
  }
  if (!isTauri) return webApi<AppSnapshot>("/daily-target", token, "PUT", { dailyCalorieTarget });
  return invoke<AppSnapshot>("update_daily_target", { token, dailyCalorieTarget });
}

export async function createFood(token: string, entry: CreateFoodInput): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return {
      ...demoSnapshot,
      foods: [...demoSnapshot.foods, { ...entry, id: crypto.randomUUID(), userId: "demo" }],
      syncStatus: "online"
    };
  }
  if (!isTauri) return webApi<AppSnapshot>("/foods", token, "POST", entry);
  return invoke<AppSnapshot>("create_food", { token, entry });
}

export async function updateFood(token: string, id: string, entry: CreateFoodInput): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return {
      ...demoSnapshot,
      foods: demoSnapshot.foods.map((item) => item.id === id ? { ...item, ...entry } : item),
      syncStatus: "online"
    };
  }
  if (!isTauri) return webApi<AppSnapshot>(`/foods/${id}`, token, "PUT", entry);
  return invoke<AppSnapshot>("update_food", { token, id, entry });
}

export async function deleteFood(token: string, id: string): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return { ...demoSnapshot, foods: demoSnapshot.foods.filter((item) => item.id !== id), syncStatus: "online" };
  }
  if (!isTauri) return webApi<AppSnapshot>(`/foods/${id}`, token, "DELETE");
  return invoke<AppSnapshot>("delete_food", { token, id });
}

export async function createExercise(token: string, entry: CreateExerciseInput): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return {
      ...demoSnapshot,
      exercises: [...demoSnapshot.exercises, { ...entry, id: crypto.randomUUID(), userId: "demo" }],
      syncStatus: "online"
    };
  }
  if (!isTauri) return webApi<AppSnapshot>("/exercises", token, "POST", entry);
  return invoke<AppSnapshot>("create_exercise", { token, entry });
}

export async function updateExercise(token: string, id: string, entry: CreateExerciseInput): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return {
      ...demoSnapshot,
      exercises: demoSnapshot.exercises.map((item) => item.id === id ? { ...item, ...entry } : item),
      syncStatus: "online"
    };
  }
  if (!isTauri) return webApi<AppSnapshot>(`/exercises/${id}`, token, "PUT", entry);
  return invoke<AppSnapshot>("update_exercise", { token, id, entry });
}

export async function deleteExercise(token: string, id: string): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return { ...demoSnapshot, exercises: demoSnapshot.exercises.filter((item) => item.id !== id), syncStatus: "online" };
  }
  if (!isTauri) return webApi<AppSnapshot>(`/exercises/${id}`, token, "DELETE");
  return invoke<AppSnapshot>("delete_exercise", { token, id });
}

export async function createWeight(token: string, entry: CreateWeightInput): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return {
      ...demoSnapshot,
      weights: [...demoSnapshot.weights, { ...entry, id: crypto.randomUUID(), userId: "demo" }],
      syncStatus: "online"
    };
  }
  if (!isTauri) return webApi<AppSnapshot>("/weights", token, "POST", entry);
  return invoke<AppSnapshot>("create_weight", { token, entry });
}

export async function updateWeight(token: string, id: string, entry: CreateWeightInput): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return {
      ...demoSnapshot,
      weights: demoSnapshot.weights.map((item) => item.id === id ? { ...item, ...entry } : item),
      syncStatus: "online"
    };
  }
  if (!isTauri) return webApi<AppSnapshot>(`/weights/${id}`, token, "PUT", entry);
  return invoke<AppSnapshot>("update_weight", { token, id, entry });
}

export async function deleteWeight(token: string, id: string): Promise<AppSnapshot> {
  if (isBrowserDev) {
    return { ...demoSnapshot, weights: demoSnapshot.weights.filter((item) => item.id !== id), syncStatus: "online" };
  }
  if (!isTauri) return webApi<AppSnapshot>(`/weights/${id}`, token, "DELETE");
  return invoke<AppSnapshot>("delete_weight", { token, id });
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
