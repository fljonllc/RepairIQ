import { Shield, Zap, Archive, Lock } from "lucide-react";

interface OnboardingProps {
  onComplete: () => void;
}

export function Onboarding({ onComplete }: OnboardingProps) {
  return (
    <div className="onboarding">
      <div className="onboarding-content">
        <div className="onboarding-logo">
          <span className="onboarding-icon">🛡️</span>
          <h1>RepairIQ</h1>
          <p className="onboarding-tagline">Your Mac's Storage Advisor</p>
        </div>

        <div className="onboarding-features">
          <div className="onboarding-feature">
            <Zap size={20} />
            <div>
              <h3>Intelligent Recommendations</h3>
              <p>Every recommendation is earned through evidence — not guessed.</p>
            </div>
          </div>
          <div className="onboarding-feature">
            <Shield size={20} />
            <div>
              <h3>Nothing Permanently Deleted</h3>
              <p>Everything goes to the Recovery Vault first. Restore anytime.</p>
            </div>
          </div>
          <div className="onboarding-feature">
            <Archive size={20} />
            <div>
              <h3>Your Data Stays Local</h3>
              <p>No cloud. No account. No telemetry. Runs 100% offline.</p>
            </div>
          </div>
          <div className="onboarding-feature">
            <Lock size={20} />
            <div>
              <h3>Protected Items Are Sacred</h3>
              <p>System files, SSH keys, and keychains are never touched.</p>
            </div>
          </div>
        </div>

        <div className="onboarding-promises">
          <h3>RepairIQ will never:</h3>
          <ul>
            <li>Delete files without your approval</li>
            <li>Use sudo or modify system files</li>
            <li>Send your data anywhere</li>
            <li>Auto-clean without confirmation</li>
          </ul>
        </div>

        <button className="onboarding-start-btn" onClick={onComplete}>
          Scan My Mac — Let's Go
        </button>
      </div>
    </div>
  );
}
