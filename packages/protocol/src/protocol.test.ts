import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { PROTOCOL_VERSION, type Observation } from './index.js';

function loadFixture(name: string): Observation {
  const filePath = resolve(__dirname, '../../../tests/fixtures', name);
  return JSON.parse(readFileSync(filePath, 'utf-8')) as Observation;
}

describe('@snippy/protocol fixtures', () => {
  it('validates browser_research.json against Observation contract', () => {
    const obs = loadFixture('browser_research.json');
    expect(obs.protocol_version).toBe(PROTOCOL_VERSION);
    expect(obs.scope.app_name).toBe('Chrome');
    expect(obs.semantic_elements.length).toBe(2);
    expect(obs.semantic_elements[0]?.is_secret).toBe(false);
  });

  it('validates adversarial_secret_field.json marks secret fields', () => {
    const obs = loadFixture('adversarial_secret_field.json');
    expect(obs.protocol_version).toBe(PROTOCOL_VERSION);
    expect(obs.semantic_elements[0]?.is_secret).toBe(true);
  });
});
