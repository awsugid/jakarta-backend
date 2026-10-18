-- Event speakers table for configuring featured speakers.
-- Stores speaker details, photo URL, company, talk title, talk format, bio,
-- social/LinkedIn URL, display order, and active/inactive status.
CREATE TABLE IF NOT EXISTS event_speakers (
  event_slug TEXT NOT NULL,
  id TEXT NOT NULL,
  name TEXT NOT NULL,
  role TEXT NOT NULL,
  company TEXT,
  photo_url TEXT NOT NULL,
  talk_title TEXT NOT NULL,
  talk_format TEXT DEFAULT 'Technical Session',
  bio TEXT,
  social_url TEXT,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  display_order INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (event_slug, id),
  UNIQUE (event_slug, display_order)
);

-- Seed initial featured speakers for Community Day 2026
INSERT INTO event_speakers
  (event_slug, id, name, role, company, photo_url, talk_title, talk_format, bio, social_url, is_active, display_order)
VALUES
  (
    'community-day-2026',
    'afif-akbar-iskandar',
    'Afif Akbar Iskandar',
    'AI/ML Specialist & Educator',
    'NgodingAI',
    'https://images.unsplash.com/photo-1534528741775-53994a69daeb?w=500&auto=format&fit=crop&q=80',
    'Prompt Engineering as Code for Production Ready AI Agents',
    'Keynote',
    'AI enthusiast and founder of NgodingAI, passionate about making machine learning and generative AI accessible and practical.',
    'https://www.linkedin.com/in/afifai/',
    1,
    1
  ),
  (
    'community-day-2026',
    'petra-novandi-barus',
    'Petra Novandi Barus',
    'Lead Developer Advocate',
    'Amazon Web Services',
    'https://images.unsplash.com/photo-1507003211169-0a1dd7228f2d?w=500&auto=format&fit=crop&q=80',
    'Architecting Serverless Microservices at Massive Scale',
    'Technical Session',
    'Veteran cloud builder with over 15 years of experience designing high-scale distributed systems and developer tooling.',
    'https://www.linkedin.com/in/petranovandi/',
    1,
    2
  ),
  (
    'community-day-2026',
    'deisya-nur-aisyah',
    'Deisya Nur Aisyah',
    'Principal Cloud Architect',
    'CloudScale Indonesia',
    'https://images.unsplash.com/photo-1573496359142-b8d87734a5a2?w=500&auto=format&fit=crop&q=80',
    'Zero-Trust Cloud Security Architecture with AWS IAM & KMS',
    'Deep Dive',
    'Cloud security strategist helping enterprises modernize and fortify mission-critical workloads in highly regulated industries.',
    'https://www.linkedin.com/company/aws-user-group-indonesia/',
    1,
    3
  ),
  (
    'community-day-2026',
    'muhammad-abdul-aziz',
    'Muhammad Abdul Aziz',
    'Staff DevOps Engineer',
    'FinTech Core',
    'https://images.unsplash.com/photo-1500648767791-00dcc994a43e?w=500&auto=format&fit=crop&q=80',
    'Automating Multi-Region Kubernetes Deployments with EKS & Karpenter',
    'Workshop',
    'Specializes in container orchestration, infrastructure as code, and auto-scaling cost optimization on AWS.',
    'https://github.com/awsugid',
    1,
    4
  )
ON CONFLICT(event_slug, id) DO NOTHING;
