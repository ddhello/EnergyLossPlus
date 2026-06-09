import { invoke } from "@tauri-apps/api/core";
import type { AppSnapshot, GoalRecommendation, ProfileInput, Session } from "./types";

const isTauri = "__TAURI_INTERNALS__" in window;

const demoSnapshot: AppSnapshot = {
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
  if (!isTauri) {
    return demoSnapshot;
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
  if (!isTauri) {
    return { ...demoSnapshot, syncStatus: "online" };
  }
  return invoke<AppSnapshot>("sync_snapshot", { token });
}

export async function saveSession(session: Session): Promise<void> {
  if (!isTauri) {
    return;
  }
  await invoke("save_session", { session });
}

export async function clearSession(): Promise<void> {
  if (!isTauri) {
    return;
  }
  await invoke("clear_session");
}

export async function updateGoal(token: string, profile: ProfileInput): Promise<AppSnapshot> {
  if (!isTauri) {
    return { ...demoSnapshot, profile, syncStatus: "online" };
  }
  return invoke<AppSnapshot>("update_goal", { token, profile });
}

export async function createFood(token: string, entry: CreateFoodInput): Promise<AppSnapshot> {
  if (!isTauri) {
    return {
      ...demoSnapshot,
      foods: [...demoSnapshot.foods, { ...entry, id: crypto.randomUUID(), userId: "demo" }],
      syncStatus: "online"
    };
  }
  return invoke<AppSnapshot>("create_food", { token, entry });
}

export async function updateFood(token: string, id: string, entry: CreateFoodInput): Promise<AppSnapshot> {
  if (!isTauri) {
    return {
      ...demoSnapshot,
      foods: demoSnapshot.foods.map((item) => item.id === id ? { ...item, ...entry } : item),
      syncStatus: "online"
    };
  }
  return invoke<AppSnapshot>("update_food", { token, id, entry });
}

export async function deleteFood(token: string, id: string): Promise<AppSnapshot> {
  if (!isTauri) {
    return { ...demoSnapshot, foods: demoSnapshot.foods.filter((item) => item.id !== id), syncStatus: "online" };
  }
  return invoke<AppSnapshot>("delete_food", { token, id });
}

export async function createExercise(token: string, entry: CreateExerciseInput): Promise<AppSnapshot> {
  if (!isTauri) {
    return {
      ...demoSnapshot,
      exercises: [...demoSnapshot.exercises, { ...entry, id: crypto.randomUUID(), userId: "demo" }],
      syncStatus: "online"
    };
  }
  return invoke<AppSnapshot>("create_exercise", { token, entry });
}

export async function updateExercise(token: string, id: string, entry: CreateExerciseInput): Promise<AppSnapshot> {
  if (!isTauri) {
    return {
      ...demoSnapshot,
      exercises: demoSnapshot.exercises.map((item) => item.id === id ? { ...item, ...entry } : item),
      syncStatus: "online"
    };
  }
  return invoke<AppSnapshot>("update_exercise", { token, id, entry });
}

export async function deleteExercise(token: string, id: string): Promise<AppSnapshot> {
  if (!isTauri) {
    return { ...demoSnapshot, exercises: demoSnapshot.exercises.filter((item) => item.id !== id), syncStatus: "online" };
  }
  return invoke<AppSnapshot>("delete_exercise", { token, id });
}

export async function createWeight(token: string, entry: CreateWeightInput): Promise<AppSnapshot> {
  if (!isTauri) {
    return {
      ...demoSnapshot,
      weights: [...demoSnapshot.weights, { ...entry, id: crypto.randomUUID(), userId: "demo" }],
      syncStatus: "online"
    };
  }
  return invoke<AppSnapshot>("create_weight", { token, entry });
}

export async function updateWeight(token: string, id: string, entry: CreateWeightInput): Promise<AppSnapshot> {
  if (!isTauri) {
    return {
      ...demoSnapshot,
      weights: demoSnapshot.weights.map((item) => item.id === id ? { ...item, ...entry } : item),
      syncStatus: "online"
    };
  }
  return invoke<AppSnapshot>("update_weight", { token, id, entry });
}

export async function deleteWeight(token: string, id: string): Promise<AppSnapshot> {
  if (!isTauri) {
    return { ...demoSnapshot, weights: demoSnapshot.weights.filter((item) => item.id !== id), syncStatus: "online" };
  }
  return invoke<AppSnapshot>("delete_weight", { token, id });
}

function today(): string {
  return new Date().toISOString().slice(0, 10);
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
