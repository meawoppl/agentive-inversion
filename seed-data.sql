-- Seed data for testing the application
-- This creates sample todos with different sources

-- Insert some manual todos
INSERT INTO todos (id, title, description, completed, source, source_id, due_date, created_at, updated_at)
VALUES
  (
    gen_random_uuid(),
    'Buy groceries',
    'Get milk, eggs, bread, and vegetables',
    false,
    'manual',
    NULL,
    NOW() + INTERVAL '2 days',
    NOW(),
    NOW()
  ),
  (
    gen_random_uuid(),
    'Finish project documentation',
    'Complete README and architecture docs',
    false,
    'manual',
    NULL,
    NOW() + INTERVAL '1 week',
    NOW(),
    NOW()
  ),
  (
    gen_random_uuid(),
    'Call dentist',
    'Schedule annual checkup',
    false,
    'manual',
    NULL,
    NOW() + INTERVAL '3 days',
    NOW(),
    NOW()
  ),
  (
    gen_random_uuid(),
    'Review pull requests',
    'Check pending PRs on GitHub',
    true,
    'manual',
    NULL,
    NOW() - INTERVAL '1 day',
    NOW() - INTERVAL '3 days',
    NOW()
  ),
  (
    gen_random_uuid(),
    'Update dependencies',
    'Run cargo update and test',
    false,
    'manual',
    NULL,
    NOW() + INTERVAL '5 days',
    NOW(),
    NOW()
  );

-- Insert sample email-sourced todos
INSERT INTO todos (id, title, description, completed, source, source_id, due_date, created_at, updated_at)
VALUES
  (
    gen_random_uuid(),
    'Respond to client inquiry',
    'Email from client about new features',
    false,
    'email',
    'email-123abc',
    NOW() + INTERVAL '1 day',
    NOW() - INTERVAL '2 hours',
    NOW() - INTERVAL '2 hours'
  ),
  (
    gen_random_uuid(),
    'Submit expense report',
    'Monthly expenses due by end of week',
    false,
    'email',
    'email-456def',
    NOW() + INTERVAL '4 days',
    NOW() - INTERVAL '1 day',
    NOW() - INTERVAL '1 day'
  );

-- Insert sample calendar-sourced todos
INSERT INTO todos (id, title, description, completed, source, source_id, due_date, created_at, updated_at)
VALUES
  (
    gen_random_uuid(),
    'Team standup meeting',
    'Daily sync at 9:30 AM',
    false,
    'calendar',
    'cal-event-001',
    NOW() + INTERVAL '1 day' + TIME '09:30:00',
    NOW(),
    NOW()
  ),
  (
    gen_random_uuid(),
    'Quarterly planning session',
    'Q1 2024 planning with leadership team',
    false,
    'calendar',
    'cal-event-002',
    NOW() + INTERVAL '1 week',
    NOW(),
    NOW()
  ),
  (
    gen_random_uuid(),
    'Coffee chat with mentor',
    'Monthly 1-on-1',
    false,
    'calendar',
    'cal-event-003',
    NOW() + INTERVAL '3 days' + TIME '15:00:00',
    NOW(),
    NOW()
  );

-- Insert sample email accounts
INSERT INTO email_accounts (id, account_name, email_address, provider, last_synced, created_at)
VALUES
  (
    gen_random_uuid(),
    'Work Email',
    'work@example.com',
    'Gmail',
    NOW() - INTERVAL '10 minutes',
    NOW() - INTERVAL '1 month'
  ),
  (
    gen_random_uuid(),
    'Personal Email',
    'personal@example.com',
    'Gmail',
    NOW() - INTERVAL '15 minutes',
    NOW() - INTERVAL '2 months'
  );

-- Insert sample calendar accounts
INSERT INTO calendar_accounts (id, account_name, calendar_id, last_synced, created_at)
VALUES
  (
    gen_random_uuid(),
    'Work Calendar',
    'work-calendar@example.com',
    NOW() - INTERVAL '5 minutes',
    NOW() - INTERVAL '1 month'
  ),
  (
    gen_random_uuid(),
    'Personal Calendar',
    'personal-calendar@example.com',
    NOW() - INTERVAL '12 minutes',
    NOW() - INTERVAL '3 months'
  );
