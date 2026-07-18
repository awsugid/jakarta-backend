-- Singleton row holding baseline community statistics JSON.
-- Manual data (Meetup era + gender/background distributions). Current-year
-- participant/event totals are computed live from Pretix and merged at read time.
CREATE TABLE IF NOT EXISTS community_statistics (
  id INTEGER PRIMARY KEY DEFAULT 1,
  data TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  CHECK (id = 1)
);

INSERT INTO community_statistics (id, data) VALUES (1, '{"participantNumOfTheYear":[{"year":2021,"total":228},{"year":2022,"total":401},{"year":2023,"total":880},{"year":2024,"total":551},{"year":2025,"total":675}],"eventPerYear":[{"year":2021,"total":8},{"year":2022,"total":10},{"year":2023,"total":5},{"year":2024,"total":4},{"year":2025,"total":6}],"participantGenderDistributionLastYear":{"male":87.9,"female":12.1},"participantBackgroundDistribution":{"professional":80.05,"student":19.85}}');
