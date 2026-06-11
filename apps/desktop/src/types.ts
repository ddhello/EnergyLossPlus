export type Sex = "female" | "male";
export type ActivityLevel = "sedentary" | "light" | "moderate" | "active" | "veryActive";
export type GoalKind = "lose" | "maintain" | "gain";

export interface ProfileInput {
  sex: Sex;
  ageYears: number;
  heightCm: number;
  weightKg: number;
  activityLevel: ActivityLevel;
  goalKind: GoalKind;
}

export interface MacroTargets {
  proteinG: number;
  carbsG: number;
  fatG: number;
}

export interface GoalRecommendation {
  bmr: number;
  tdee: number;
  dailyCalorieTarget: number;
  macros: MacroTargets;
}

export interface FoodEntry {
  id: string;
  userId: string;
  date: string;
  meal: string;
  name: string;
  calories: number;
  proteinG: number;
  carbsG: number;
  fatG: number;
  note?: string;
}

export interface ExerciseEntry {
  id: string;
  userId: string;
  date: string;
  name: string;
  caloriesBurned: number;
  durationMinutes: number;
  note?: string;
}

export interface WeightEntry {
  id: string;
  userId: string;
  date: string;
  weightKg: number;
  note?: string;
}

export interface Session {
  token: string;
  userId: string;
  nickname: string;
  deviceName: string;
  expiresAt: string;
}

export interface AppSnapshot {
  session?: Session;
  profile: ProfileInput;
  recommendation?: GoalRecommendation;
  dailyCalorieTarget?: number;
  foods: FoodEntry[];
  exercises: ExerciseEntry[];
  weights: WeightEntry[];
  syncStatus: "online" | "cached" | "offline";
}
