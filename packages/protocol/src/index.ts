export const PROTOCOL_VERSION = 1 as const;

export type TargetKind = 'window' | 'browser_tab' | 'companion_context';

export interface Scope {
  scope_id: string;
  target_kind: TargetKind;
  app_name: string;
  window_title: string;
  process_id: number | null;
}

export interface Bounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface SemanticElement {
  locator: string;
  role: string;
  label: string;
  bounds: Bounds | null;
  is_secret: boolean;
  is_enabled: boolean;
}

export interface Observation {
  protocol_version: number;
  scope: Scope;
  active_window_proof: string;
  semantic_elements: SemanticElement[];
  visual_context_summary: string | null;
  observation_hash: string;
  observed_at: string;
}

export type ActionKind =
  | 'focus'
  | 'invoke'
  | 'set_non_secret_value'
  | 'key_press'
  | 'pointer'
  | 'scroll'
  | 'wait'
  | 'request_user_takeover';

export type RiskCategory =
  | 'observe'
  | 'navigate'
  | 'draft'
  | 'local_change'
  | 'external_impact'
  | 'sensitive';

export interface CoordinateHint {
  x: number;
  y: number;
}

export interface TargetLocator {
  observation_hash: string;
  semantic_locator: string | null;
  coordinate_hint: CoordinateHint | null;
}

export interface ApprovedAction {
  protocol_version: number;
  action_id: string;
  scope_id: string;
  observation_hash: string;
  kind: ActionKind;
  target: TargetLocator;
  arguments: Record<string, unknown>;
  risk: RiskCategory;
  reason: string;
  expected_postcondition: string;
  expires_at: string;
}

export type Verification =
  | { status: 'confirmed'; evidence: string }
  | { status: 'not_confirmed'; reason: string }
  | { status: 'ambiguous'; reason: string }
  | { status: 'blocked'; reason: string };

export type SessionStatus =
  | 'ready'
  | 'inspecting'
  | 'planning'
  | 'acting'
  | 'awaiting_approval'
  | 'awaiting_secret'
  | 'blocked'
  | 'complete'
  | 'stopped';

export interface TimelineStep {
  step_number: number;
  timestamp: string;
  action_summary: string;
  target_label: string;
  risk: RiskCategory;
  observation_hash: string;
  policy_decision: string;
  verification: Verification | null;
}

export interface SessionRecord {
  session_id: string;
  task: string;
  scope: Scope;
  status: SessionStatus;
  steps: TimelineStep[];
  created_at: string;
  updated_at: string;
}
