// Generated types come from crates/corde-protocol (run `pnpm protocol:gen`).
// This file re-exports them and adds a few constants shared by every client.

export type { Aero } from "./generated/Aero";
export type { ClientMessage } from "./generated/ClientMessage";
export type { CompoundType } from "./generated/CompoundType";
export type { Control } from "./generated/Control";
export type { Damage } from "./generated/Damage";
export type { DashboardLayout } from "./generated/DashboardLayout";
export type { DeviceKind } from "./generated/DeviceKind";
export type { Electronics } from "./generated/Electronics";
export type { Energy } from "./generated/Energy";
export type { Engine } from "./generated/Engine";
export type { FinishStatus } from "./generated/FinishStatus";
export type { GameEvent } from "./generated/GameEvent";
export type { GameId } from "./generated/GameId";
export type { GamePhase } from "./generated/GamePhase";
export type { Gaps } from "./generated/Gaps";
export type { Inputs } from "./generated/Inputs";
export type { LapNote } from "./generated/LapNote";
export type { LapRecord } from "./generated/LapRecord";
export type { LapTiming } from "./generated/LapTiming";
export type { Motion } from "./generated/Motion";
export type { MotorState } from "./generated/MotorState";
export type { PitState } from "./generated/PitState";
export type { PlayerTelemetry } from "./generated/PlayerTelemetry";
export type { RaceReminder } from "./generated/RaceReminder";
export type { RaceReminderKind } from "./generated/RaceReminderKind";
export type { ServerMessage } from "./generated/ServerMessage";
export type { SessionInfo } from "./generated/SessionInfo";
export type { SessionType } from "./generated/SessionType";
export type { SourceStatus } from "./generated/SourceStatus";
export type { Standing } from "./generated/Standing";
export type { SurfaceType } from "./generated/SurfaceType";
export type { TelemetryFrame } from "./generated/TelemetryFrame";
export type { TyreSet } from "./generated/TyreSet";
export type { VehicleClass } from "./generated/VehicleClass";
export type { Weather } from "./generated/Weather";
export type { Wheel } from "./generated/Wheel";
export type { WidgetPlacement } from "./generated/WidgetPlacement";
export type { YellowFlagState } from "./generated/YellowFlagState";

// Post-session analysis (corde-engineer, GET /api/sessions/{id}/analysis).
export type { AnalysisReport } from "./generated/AnalysisReport";
export type { BalanceAnalysis } from "./generated/BalanceAnalysis";
export type { BalanceCell } from "./generated/BalanceCell";
export type { BalanceMatrix } from "./generated/BalanceMatrix";
export type { BalanceReference } from "./generated/BalanceReference";
export type { BalanceRow } from "./generated/BalanceRow";
export type { BrakeAnalysis } from "./generated/BrakeAnalysis";
export type { BrakeWheelAnalysis } from "./generated/BrakeWheelAnalysis";
export type { BrakeWindow } from "./generated/BrakeWindow";
export type { CamberHint } from "./generated/CamberHint";
export type { ChangeDirection } from "./generated/ChangeDirection";
export type { CornerPhase } from "./generated/CornerPhase";
export type { CornerSpeed } from "./generated/CornerSpeed";
export type { CornerStat } from "./generated/CornerStat";
export type { DamperHist } from "./generated/DamperHist";
export type { DataQuality } from "./generated/DataQuality";
export type { DrivingStyle } from "./generated/DrivingStyle";
export type { Evidence } from "./generated/Evidence";
export type { Finding } from "./generated/Finding";
export type { LapStats } from "./generated/LapStats";
export type { SetupChange } from "./generated/SetupChange";
export type { SetupPhilosophy } from "./generated/SetupPhilosophy";
export type { SetupPlan } from "./generated/SetupPlan";
export type { SetupPlans } from "./generated/SetupPlans";
export type { Severity } from "./generated/Severity";
export type { SuspensionAnalysis } from "./generated/SuspensionAnalysis";
export type { TurnDirection } from "./generated/TurnDirection";
export type { TyreAnalysis } from "./generated/TyreAnalysis";
export type { TyreWheelAnalysis } from "./generated/TyreWheelAnalysis";

// Driver feedback (GET / POST /api/sessions/{id}/feedback), its interpretation and the plan
// iterations it drives (GET /api/sessions/{id}/iterations).
export type { Axle } from "./generated/Axle";
export type { DetectedSymptom } from "./generated/DetectedSymptom";
export type { ExtractorKind } from "./generated/ExtractorKind";
export type { Feedback } from "./generated/Feedback";
export type { FeedbackInterpretation } from "./generated/FeedbackInterpretation";
export type { NewFeedback } from "./generated/NewFeedback";
export type { PlanIteration } from "./generated/PlanIteration";
export type { SymptomIntensity } from "./generated/SymptomIntensity";

/** Must match corde_protocol::PROTOCOL_VERSION. */
export const PROTOCOL_VERSION = 1;
/** Default HTTP + WebSocket port of corde-server. */
export const DEFAULT_PORT = 8471;
/** WebSocket path. */
export const WS_PATH = "/ws";

/** Wheel order used everywhere: front-left, front-right, rear-left, rear-right. */
export const WHEELS = ["FL", "FR", "RL", "RR"] as const;
export type WheelIndex = 0 | 1 | 2 | 3;

// Real setups: the driver's .svm files, plans resolved against one of them and the files Corde
// writes (GET /api/setups, GET /api/sessions/{id}/setup-preview, POST …/setup-files).
export type { CreatedSetupFile } from "./generated/CreatedSetupFile";
export type { NewSetupFiles } from "./generated/NewSetupFiles";
export type { ResolvedChange } from "./generated/ResolvedChange";
export type { ResolvedValue } from "./generated/ResolvedValue";
export type { ResolveStatus } from "./generated/ResolveStatus";
export type { SetupFileInfo } from "./generated/SetupFileInfo";
export type { SetupFilesCreated } from "./generated/SetupFilesCreated";
export type { SetupLibrary } from "./generated/SetupLibrary";
export type { SetupPreview } from "./generated/SetupPreview";
