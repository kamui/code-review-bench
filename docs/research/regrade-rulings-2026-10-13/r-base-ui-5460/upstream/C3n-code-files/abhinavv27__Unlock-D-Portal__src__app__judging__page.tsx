'use client'

import { useState, useEffect, useCallback, useMemo } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import Link from 'next/link'
import { useRouter } from 'next/navigation'
import { Slider } from '@base-ui/react/slider'
import sliderStyles from './JudgeSlider.module.css'
import { api } from '@/trpc/react'

function getFeatureLabel(taskId: string): string {
  if (!taskId) return ''
  if (taskId === 'FINAL-FEATURE') return 'Final Feature'
  if (taskId.startsWith('FEATURE-')) return `Feature ${taskId.split('-')[1]}`
  if (taskId.startsWith('ROUND-')) return `Round ${taskId.split('-')[1]}`
  return taskId
}

function timeAgo(dateStr: string): string {
  const now = Date.now()
  const then = new Date(dateStr).getTime()
  const diff = now - then
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins} min ago`
  const hrs = Math.floor(mins / 60)
  const rem = mins % 60
  return `${hrs}h ${rem}m ago`
}

// CRITERIA and INITIAL_SCORES have been moved to the backend.
// The frontend dynamically uses the criteria provided in the queue API.

export default function JudgingPage() {
  const router = useRouter()
  const [mounted, setMounted] = useState(false)
  const [staffToken, setStaffToken] = useState<string | null>(null)

  // Queue
  const [queue, setQueue] = useState<any[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  // Active submission
  const [activeSubmission, setActiveSubmission] = useState<any | null>(null)
  const [scores, setScores] = useState<Record<string, number>>({})

  const currentCriteria = useMemo(() => {
    return activeSubmission?.criteria || []
  }, [activeSubmission])

  const initializeScoresForSubmission = useCallback((sub: any): Record<string, number> => {
    const initial: Record<string, number> = {}
    if (sub?.criteria && Array.isArray(sub.criteria)) {
      sub.criteria.forEach((c: any) => {
        initial[c.key] = Math.max(0, Math.round((c.max || 10) / 2))
      })
    }
    return initial
  }, [])
  const [notes, setNotes] = useState('')
  const [gradeStatus, setGradeStatus] = useState<'APPROVED' | 'REJECTED'>('APPROVED')
  const [submitLoading, setSubmitLoading] = useState(false)
  const [submitSuccess, setSubmitSuccess] = useState(false)
  const [sidebarOpen, setSidebarOpen] = useState(false)

  // View toggle
  const [mainView, setMainView] = useState<'queue' | 'feedback' | 'logs'>('queue')

  // Leaderboard visibility state and actions
  const utils = api.useUtils()
  const { data: visibilityData } = api.judging.getLeaderboardVisibility.useQuery(undefined, {
    enabled: mounted && !!staffToken
  })
  const toggleVisibility = api.judging.toggleLeaderboardVisibility.useMutation({
    onSuccess: () => {
      void utils.judging.getLeaderboardVisibility.invalidate()
    }
  })
  const isPublicVisible = visibilityData?.isVisible ?? false

  // Team Logs
  const [teamLogs, setTeamLogs] = useState<any[]>([])
  const [teamLogsLoading, setTeamLogsLoading] = useState(false)
  const [currentGlobalRound, setCurrentGlobalRound] = useState(1)
  const [teamSearch, setTeamSearch] = useState('')
  const [queueSearch, setQueueSearch] = useState('')
  const [queueFeatureSearch, setQueueFeatureSearch] = useState('')
  const [expandedTeam, setExpandedTeam] = useState<string | null>(null)

  // Current logged in user info
  const [staffUser, setStaffUser] = useState<{ userId: number; username: string; role: string } | null>(null)



  const fetchQueue = useCallback(async (token: string) => {
    setLoading(true)
    setError(null)
    try {
      const res = await fetch('/api/admin/queue', {
        headers: { Authorization: `Bearer ${token}` },
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || 'Failed to retrieve queue.')
      setQueue(data.submissions || [])
    } catch (err: any) {
      setError(err.message || 'Failed to load queue.')
    } finally {
      setLoading(false)
    }
  }, [])



  const fetchTeamLogs = useCallback(async (token: string) => {
    setTeamLogsLoading(true)
    try {
      const res = await fetch('/api/admin/team-logs', {
        headers: { Authorization: `Bearer ${token}` },
      })
      const data = await res.json()
      if (res.ok) {
        setTeamLogs(data.teams || [])
        setCurrentGlobalRound(data.currentGlobalRound || 1)
      }
    } catch {
      // silently fail
    } finally {
      setTeamLogsLoading(false)
    }
  }, [])

  const fetchStaffUser = useCallback(async (token: string) => {
    try {
      const res = await fetch('/api/auth/staff/me', {
        headers: { Authorization: `Bearer ${token}` },
      })
      if (res.ok) {
        const data = await res.json()
        setStaffUser(data)
      }
    } catch (err) {
      console.error('Failed to fetch staff details:', err)
    }
  }, [])



  useEffect(() => {
    setMounted(true)
    let token = localStorage.getItem('staff_token')
    if (!token) {
      const match = document.cookie.match(/staff_token=([^;]+)/)
      if (match) {
        token = decodeURIComponent(match[1])
        localStorage.setItem('staff_token', token)
      }
    }

    if (!token) {
      router.push('/login')
      return
    }
    setStaffToken(token)
    fetchQueue(token)
    fetchTeamLogs(token)
    fetchStaffUser(token)
  }, [router, fetchQueue, fetchTeamLogs, fetchStaffUser])

  // Auto-refresh review queue & team logs every 30 seconds
  useEffect(() => {
    if (!staffToken) return
    const interval = setInterval(() => {
      fetchQueue(staffToken)
      fetchTeamLogs(staffToken)
    }, 30000)
    return () => clearInterval(interval)
  }, [staffToken, fetchQueue, fetchTeamLogs])

  const handleGradeSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!activeSubmission || !staffToken) return

    setSubmitLoading(true)
    setError(null)
    setSubmitSuccess(false)

    try {
      const res = await fetch('/api/admin/grade', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${staffToken}`,
        },
        body: JSON.stringify({
          submissionId: activeSubmission.id,
          scoreBreakdown: scores,
          feedback: notes,
          status: gradeStatus,
        }),
      })

      const data = await res.json()
      if (!res.ok) throw new Error(data.error || 'Failed to submit grade.')

      setSubmitSuccess(true)
      setQueue((prev) => prev.filter((sub) => sub.id !== activeSubmission.id))
      setActiveSubmission(null)
      setScores({})
      setNotes('')
      setGradeStatus('APPROVED')
      if (staffToken) {
        fetchTeamLogs(staffToken)
      }
    } catch (err: any) {
      setError(err.message || 'Failed to save scores.')
    } finally {
      setSubmitLoading(false)
    }
  }

  const overall = useMemo(() => {
    return (
      currentCriteria.reduce((sum: number, c: any) => sum + (scores[c.key] || 0), 0)
    ).toFixed(0)
  }, [currentCriteria, scores])

  const filteredTeams = useMemo(() => {
    if (!teamSearch.trim()) return teamLogs
    const q = teamSearch.toLowerCase()
    return teamLogs.filter((t: any) => t.teamName.toLowerCase().includes(q))
  }, [teamLogs, teamSearch])

  const uniqueFeatures = useMemo(() => {
    if (currentGlobalRound === 1) {
      return [
        'Feature 1',
        'Feature 2',
        'Feature 3',
        'Feature 4',
        'Feature 5',
        'Final Submission'
      ]
    }
    if (currentGlobalRound === 2) {
      return ['Round 2']
    }
    if (currentGlobalRound === 3) {
      return ['Round 3']
    }
    return []
  }, [currentGlobalRound])

  // Helper: get current judge's userId
  const getStaffUserId = useCallback(() => {
    return staffUser?.userId ?? null
  }, [staffUser])

  const handleSelectTeamLogSubmission = (sub: any, team: any) => {
    const judgeId = getStaffUserId()
    const myEval = sub.evaluations?.find((e: any) => e.judgeId === judgeId)

    // Build a submission object compatible with the grading form
    const formattedSub = {
      ...sub,
      registration: {
        teamName: team.teamName,
        progressState: { current_stage: team.currentStage },
      },
    }

    setActiveSubmission(formattedSub)
    setSubmitSuccess(false)

    if (myEval) {
      // Pre-populate with existing evaluation
      const breakdown = myEval.scoreBreakdown || {}
      if (sub.taskId.startsWith('FEATURE-')) {
        setScores({})
      } else {
        setScores({
          functionality: Number(breakdown.functionality) || 0,
          codeQuality: Number(breakdown.codeQuality) || 0,
          integration: Number(breakdown.integration) || 0,
          userExperience: Number(breakdown.userExperience) || 0,
          deployment: Number(breakdown.deployment) || 0,
          teamwork: Number(breakdown.teamwork) || 0,
          errorHandling: Number(breakdown.errorHandling) || 0,
        })
      }
      setNotes(myEval.feedback || '')
    } else {
      setScores(initializeScoresForSubmission(sub))
      setNotes('')
    }
    setGradeStatus('APPROVED')
    setMainView('queue')
    setSidebarOpen(false)
  }

  if (!mounted) return <div className="min-h-screen bg-[#050505]" />

  return (
    <main className="min-h-screen flex bg-[#050505] text-white selection:bg-primary relative overflow-x-hidden font-sans">
      {/* Background */}
      <div className="fixed inset-0 pointer-events-none z-0">
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_0%,#1a1a1a,transparent_70%)]" />
        <div className="absolute top-0 left-0 w-full h-full neural-grid opacity-[0.03]" />
      </div>

      {/* Mobile Menu */}
      <button
        onClick={() => setSidebarOpen(!sidebarOpen)}
        className="lg:hidden fixed top-4 left-4 z-50 p-3 rounded-xl bg-black/60 border border-white/10 backdrop-blur-xl"
      >
        <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      </button>

      {sidebarOpen && (
        <div className="lg:hidden fixed inset-0 bg-black/60 z-40" onClick={() => setSidebarOpen(false)} />
      )}

      {/* Sidebar — Queue + Team Logs */}
      <aside className={`fixed lg:sticky top-0 left-0 h-screen w-80 border-r border-white/5 bg-black/80 lg:bg-black/40 backdrop-blur-3xl flex flex-col z-50 lg:z-10 transition-transform duration-300 ${sidebarOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}`}>
        {/* Header */}
        <div className="p-6 md:p-8 border-b border-white/5 space-y-4">
          <Link href="/dashboard" className="flex items-center gap-4 group">
            <div className="w-10 h-10 rounded-xl overflow-hidden flex items-center justify-center group-hover:scale-110 transition-transform shadow-xl shadow-white/5 border border-white/10">
              <img src="/ras-logo.png" alt="RAS Logo" className="w-full h-full object-contain" />
            </div>
            <div className="flex flex-col">
              <span className="text-label-caps !text-[10px] text-white">Judge Console</span>
              <span className="text-[8px] font-mono text-white/40 tracking-wider">UNLOCK&apos;D // IEEE RAS</span>
            </div>
          </Link>

          <div className="flex justify-between items-end">
            <p className="text-label-caps !text-[9px] text-white/30">
              {mainView === 'feedback' ? 'Feedback Queue' : 'Submission Queue'}
            </p>
            <p className="text-value-mono !text-[10px] text-primary">
              {queue.filter(sub => {
                const isFeature = sub.taskId.startsWith('FEATURE-')
                return mainView === 'feedback' ? isFeature : !isFeature
              }).length} pending
            </p>
          </div>

          {/* View Toggle */}
          <div className="flex rounded-xl border border-white/10 overflow-hidden mt-4">
            <button
              onClick={() => {
                setMainView('queue')
                setQueueSearch('')
                setQueueFeatureSearch('')
              }}
              className={`flex-1 py-2 text-[8px] font-bold uppercase tracking-wider transition-all ${
                mainView === 'queue'
                  ? 'bg-primary/10 text-primary border-r border-primary/20'
                  : 'bg-white/[0.02] text-white/40 hover:text-white/60 border-r border-white/10'
              }`}
            >
              Queue
            </button>
            <button
              onClick={() => {
                setMainView('feedback')
                setQueueSearch('')
                setQueueFeatureSearch('')
              }}
              className={`flex-1 py-2 text-[8px] font-bold uppercase tracking-wider transition-all ${
                mainView === 'feedback'
                  ? 'bg-sky-500/10 text-sky-400 border-r border-sky-500/20'
                  : 'bg-white/[0.02] text-white/40 hover:text-white/60 border-r border-white/10'
              }`}
            >
              Feedback
            </button>
            <button
              onClick={() => {
                setMainView('logs')
                setQueueSearch('')
                setQueueFeatureSearch('')
                if (staffToken) fetchTeamLogs(staffToken)
              }}
              className={`flex-1 py-2 text-[8px] font-bold uppercase tracking-wider transition-all ${
                mainView === 'logs'
                  ? 'bg-violet-500/10 text-violet-300'
                  : 'bg-white/[0.02] text-white/40 hover:text-white/60'
              }`}
            >
              Logs
            </button>
          </div>

          {/* Queue Search Input */}
          {(mainView === 'queue' || mainView === 'feedback') && (
            <div className="space-y-2 mt-4">
              {/* Team Name Filter */}
              <div className="relative">
                <input
                  type="text"
                  value={queueSearch}
                  onChange={(e) => setQueueSearch(e.target.value)}
                  placeholder="Filter by Team Name..."
                  className="w-full bg-white/[0.03] border border-white/10 rounded-xl pl-8 pr-8 py-2 text-[10px] text-white/75 focus:outline-none focus:border-primary/50 focus:bg-white/[0.06] transition-all placeholder:text-white/20 font-mono"
                />
                <div className="absolute left-2.5 top-1/2 -translate-y-1/2 text-white/30 pointer-events-none">
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                  </svg>
                </div>
                {queueSearch && (
                  <button
                    onClick={() => setQueueSearch('')}
                    className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[10px] text-white/30 hover:text-white/70 transition-colors"
                  >
                    ✕
                  </button>
                )}
              </div>

              {/* Feature/Task Name Filter Dropdown */}
              <div className="relative">
                <select
                  value={queueFeatureSearch}
                  onChange={(e) => {
                    const val = e.target.value
                    setQueueFeatureSearch(val)
                    if (val) {
                      const isFeedbackTask = ['Feature 1', 'Feature 2', 'Feature 3', 'Feature 4', 'Feature 5'].includes(val)
                      if (isFeedbackTask) {
                        setMainView('feedback')
                      } else {
                        setMainView('queue')
                      }
                    }
                  }}
                  className="w-full bg-white/[0.03] border border-white/10 rounded-xl pl-8 pr-10 py-2 text-[10px] text-white/75 focus:outline-none focus:border-primary/50 focus:bg-white/[0.06] transition-all font-mono appearance-none"
                >
                  <option value="" className="bg-[#050505] text-white/40">All Features/Tasks</option>
                  {uniqueFeatures.map(feat => (
                    <option key={feat} value={feat} className="bg-[#050505] text-white">
                      {feat}
                    </option>
                  ))}
                </select>
                <div className="absolute left-2.5 top-1/2 -translate-y-1/2 text-white/30 pointer-events-none">
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
                  </svg>
                </div>
                <div className="absolute right-2.5 top-1/2 -translate-y-1/2 text-white/30 pointer-events-none">
                  <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                  </svg>
                </div>
              </div>
            </div>
          )}
        </div>

        <div className="flex-1 overflow-auto p-4 md:p-5 space-y-2 custom-scrollbar min-h-0">
          {(() => {
            const displayQueue = queue
              .filter(sub => {
                if (queueFeatureSearch) {
                  if (queueFeatureSearch === 'Final Submission') {
                    return sub.taskId === 'FINAL-FEATURE'
                  }
                  const label = getFeatureLabel(sub.taskId)
                  return label.toLowerCase() === queueFeatureSearch.toLowerCase()
                }
                const isFeature = sub.taskId.startsWith('FEATURE-')
                return mainView === 'feedback' ? isFeature : !isFeature
              })
              .filter(sub => {
                const matchTeam = !queueSearch.trim() || sub.registration.teamName.toLowerCase().includes(queueSearch.toLowerCase())
                const featureLabel = getFeatureLabel(sub.taskId)
                const matchFeature = !queueFeatureSearch.trim() || 
                  (queueFeatureSearch === 'Final Submission' ? sub.taskId === 'FINAL-FEATURE' : featureLabel.toLowerCase().includes(queueFeatureSearch.toLowerCase()))
                return matchTeam && matchFeature
              })
            
            if (loading) {
              return <div className="text-center py-16 text-white/20 text-xs font-mono tracking-widest">LOADING...</div>
            }
            if (displayQueue.length === 0) {
              return (
                <div className="text-center py-16 text-white/20 text-xs leading-relaxed font-mono">
                  ALL CLEAR<br />No pending submissions.
                </div>
              )
            }
            return displayQueue.map((sub, idx) => (
              <motion.button
                key={sub.id}
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: idx * 0.03 }}
                onClick={() => {
                  setActiveSubmission(sub)
                  setSubmitSuccess(false)
                  setScores(initializeScoresForSubmission(sub))
                  setNotes('')
                  setGradeStatus('APPROVED')
                  setSidebarOpen(false)
                }}
                className={`w-full text-left rounded-2xl p-4 border transition-all duration-200 group ${
                  activeSubmission?.id === sub.id
                    ? 'border-primary/50 bg-primary/5 shadow-lg shadow-primary/5'
                    : 'border-white/5 hover:border-white/15 hover:bg-white/[0.02]'
                }`}
              >
                <div className="flex items-center justify-between mb-2">
                  <span className="text-label-caps !text-[8px] text-white/30">
                    {getFeatureLabel(sub.taskId)}
                  </span>
                  <span className="text-[8px] font-mono text-white/20">
                    {timeAgo(sub.submittedAt)}
                  </span>
                </div>
                <h3 className="text-headline !text-sm !not-italic group-hover:text-primary transition-colors uppercase tracking-tight">
                  {sub.registration.teamName}
                </h3>
                <p className="text-label-caps !text-[8px] mt-1 opacity-30 truncate">
                  {sub.payload?.github || 'No link'}
                </p>
              </motion.button>
            ))
          })()}
        </div>



        {/* Bottom actions */}
        <div className="p-4 md:p-5 border-t border-white/5 flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <button
              onClick={() => staffToken && fetchQueue(staffToken)}
              className="text-[8px] font-black text-white/30 hover:text-white uppercase tracking-[0.2em]"
            >
              Refresh Queue
            </button>
            <span className="text-[7px] font-mono text-white/10 uppercase">Panel Active</span>
          </div>
          {(staffUser?.role === 'ADMIN' || staffUser?.role === 'JUDGE') && (
            <>
              <div className="flex items-center justify-between bg-white/5 border border-white/10 rounded-xl px-4 py-2 mt-1 mb-1">
                <div className="flex flex-col text-left">
                  <span className="text-[9px] font-bold text-white uppercase tracking-wider">Public Leaderboard</span>
                  <span className="text-[7px] text-white/40 uppercase tracking-widest">
                    {isPublicVisible ? 'Visible' : 'Hidden'}
                  </span>
                </div>
                <button
                  onClick={() => toggleVisibility.mutate({ isVisible: !isPublicVisible })}
                  disabled={toggleVisibility.isPending}
                  className={`w-10 h-6 rounded-full p-0.5 transition-colors duration-300 ${isPublicVisible ? 'bg-primary' : 'bg-white/10'} disabled:opacity-50 cursor-pointer`}
                >
                  <div className={`w-5 h-5 bg-white rounded-full transition-transform duration-300 ${isPublicVisible ? 'translate-x-4' : 'translate-x-0'}`} />
                </button>
              </div>
              <Link
                href="/admin"
                className="w-full btn-ghost py-2 rounded-xl text-[8px] font-mono tracking-wider hover:bg-primary/10 hover:border-primary/30 hover:text-primary text-center uppercase block"
              >
                Admin Dashboard
              </Link>
            </>
          )}
          <button
            onClick={() => {
              localStorage.removeItem('staff_token')
              localStorage.removeItem('team_token')
              window.location.href = '/api/auth/logout'
            }}
            className="w-full btn-ghost py-2 rounded-xl text-[8px] font-mono tracking-wider hover:bg-rose-500/10 hover:border-rose-500/30 hover:text-rose-400 text-center uppercase"
          >
            Sign Out
          </button>
        </div>
      </aside>

      {/* Main content area */}
      <div className="flex-1 relative z-10 overflow-auto">
        {error && (
          <div className="max-w-4xl mx-auto mt-6 px-6">
            <div className="p-4 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs text-center">
              {error}
            </div>
          </div>
        )}
        {submitSuccess && (
          <div className="max-w-4xl mx-auto mt-6 px-6">
            <div className="p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-xs text-center">
              Scores saved successfully! Team progression updated.
            </div>
          </div>
        )}

        {/* ===== TEAM LOGS VIEW ===== */}
        {mainView === 'logs' && (
          <div className="max-w-5xl mx-auto p-6 md:p-8 lg:p-10 space-y-6">
            <div className="space-y-3">
              <div className="flex items-center gap-3">
                <span className="px-3 py-1.5 rounded-full bg-violet-500/10 text-violet-300 text-[9px] font-bold uppercase tracking-wider border border-violet-500/20">
                  Round {currentGlobalRound}
                </span>
                <h2 className="text-3xl md:text-5xl font-display font-bold text-white tracking-tight">
                  Team Logs
                </h2>
              </div>
              <p className="text-sm text-white/30 max-w-lg">
                View all teams, their submission history, and grade or re-grade submissions for the current round.
              </p>
            </div>

            {/* Search */}
            <div className="relative">
              <input
                type="text"
                value={teamSearch}
                onChange={(e) => setTeamSearch(e.target.value)}
                placeholder="Search teams by name..."
                className="w-full bg-white/[0.03] border border-white/10 rounded-xl px-5 py-3.5 text-sm text-white/75 focus:outline-none focus:border-violet-400/40 placeholder:text-white/15"
              />
              <span className="absolute right-4 top-1/2 -translate-y-1/2 text-[10px] font-mono text-white/20">
                {filteredTeams.length} teams
              </span>
            </div>

            {/* Teams Grid */}
            {teamLogsLoading ? (
              <div className="text-center py-16 text-white/20 text-xs font-mono animate-pulse tracking-widest">LOADING TEAM LOGS...</div>
            ) : filteredTeams.length === 0 ? (
              <div className="text-center py-16 text-white/20 text-xs font-mono">No teams found.</div>
            ) : (
              <div className="space-y-3">
                {filteredTeams.map((team: any) => {
                  const isExpanded = expandedTeam === team.id
                  return (
                    <div key={team.id} className="rounded-2xl border border-white/5 bg-white/[0.01] overflow-hidden transition-all">
                      {/* Team header row */}
                      <button
                        onClick={() => setExpandedTeam(isExpanded ? null : team.id)}
                        className="w-full flex items-center justify-between px-5 py-4 hover:bg-white/[0.02] transition-colors text-left"
                      >
                        <div className="flex items-center gap-4 min-w-0">
                          <div className="w-8 h-8 rounded-lg bg-violet-500/10 border border-violet-500/20 flex items-center justify-center text-violet-300 text-xs font-bold shrink-0">
                            {team.currentStage}
                          </div>
                          <div className="min-w-0">
                            <h4 className="text-sm font-semibold text-white truncate">{team.teamName}</h4>
                            <p className="text-[10px] font-mono text-white/25">
                              {team.submissions.length} submissions · {team.totalScore} pts
                            </p>
                          </div>
                        </div>
                        <div className="flex items-center gap-3">
                          <span className="text-[10px] font-mono text-white/20">Stage {team.currentStage}</span>
                          <svg
                            className={`w-4 h-4 text-white/30 transition-transform ${isExpanded ? 'rotate-180' : ''}`}
                            fill="none" stroke="currentColor" viewBox="0 0 24 24"
                          >
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                          </svg>
                        </div>
                      </button>

                      {/* Expanded submissions */}
                      {isExpanded && (
                        <div className="border-t border-white/5 bg-white/[0.01]">
                          {team.submissions.length === 0 ? (
                            <div className="px-5 py-6 text-xs text-white/20 font-mono text-center">No submissions yet.</div>
                          ) : (
                            <div className="divide-y divide-white/5">
                              {team.submissions.map((sub: any) => {
                                const isCurrentRound = sub.roundNumber === currentGlobalRound
                                const judgeId = getStaffUserId()
                                const myEval = sub.evaluations?.find((e: any) => e.judgeId === judgeId)

                                return (
                                  <div key={sub.id} className="px-5 py-4 flex flex-col md:flex-row md:items-center gap-4">
                                    <div className="flex-1 min-w-0 space-y-1">
                                      <div className="flex items-center gap-2 flex-wrap">
                                        <span className="text-[9px] font-mono px-2 py-0.5 rounded-md border border-white/10 bg-white/[0.03] text-white/40">
                                          R{sub.roundNumber}
                                        </span>
                                        <span className={`text-[9px] font-mono px-2 py-0.5 rounded-md border ${
                                          sub.status === 'APPROVED'
                                            ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/20'
                                            : sub.status === 'REJECTED'
                                              ? 'bg-rose-500/10 text-rose-300 border-rose-500/20'
                                              : 'bg-amber-500/10 text-amber-300 border-amber-500/20'
                                        }`}>
                                          {sub.status}
                                        </span>
                                        <span className="text-[9px] font-mono px-2 py-0.5 rounded-md border border-white/5 text-white/25 uppercase">
                                          {sub.submission_type}
                                        </span>
                                        {sub.averageScore !== null && sub.averageScore !== undefined && (
                                          <span className="text-[9px] font-mono text-primary">
                                            avg: {sub.averageScore}
                                          </span>
                                        )}
                                      </div>
                                      <p className="text-[10px] text-white/25 font-mono truncate">
                                        Task: {getFeatureLabel(sub.taskId)} · {new Date(sub.submittedAt).toLocaleString()}
                                      </p>
                                      {sub.evaluations.length > 0 && (
                                        <div className="flex flex-wrap gap-2 mt-1">
                                          {sub.evaluations.map((ev: any) => (
                                            <span
                                              key={ev.id}
                                              className={`text-[8px] font-mono px-2 py-0.5 rounded border ${
                                                ev.judgeId === judgeId
                                                  ? 'bg-primary/10 text-primary border-primary/20'
                                                  : 'bg-white/5 text-white/30 border-white/5'
                                              }`}
                                            >
                                              {ev.judgeName}: {ev.totalScore}pts
                                            </span>
                                          ))}
                                        </div>
                                      )}
                                    </div>

                                    <div className="shrink-0">
                                      {isCurrentRound ? (
                                        <button
                                          onClick={() => handleSelectTeamLogSubmission(sub, team)}
                                          className={`px-4 py-2 rounded-xl text-[9px] font-bold uppercase tracking-wider transition-all ${
                                            myEval
                                              ? 'border border-violet-500/30 bg-violet-500/10 text-violet-300 hover:bg-violet-500/20'
                                              : 'border border-primary/30 bg-primary/10 text-primary hover:bg-primary/20'
                                          }`}
                                        >
                                          {myEval ? 'Edit Grade' : 'Grade'}
                                        </button>
                                      ) : (
                                        <span className="px-4 py-2 rounded-xl text-[9px] font-mono text-white/15 border border-white/5 bg-white/[0.01] cursor-not-allowed">
                                          Round Closed
                                        </span>
                                      )}
                                    </div>
                                  </div>
                                )
                              })}
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        )}

        {/* ===== GRADING QUEUE VIEW ===== */}
        {(mainView === 'queue' || mainView === 'feedback') && (
        <AnimatePresence mode="wait">
          {activeSubmission ? (
            <motion.div
              key={activeSubmission.id}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              className="max-w-4xl mx-auto p-6 md:p-8 lg:p-10 space-y-8"
            >
              {/* Header */}
              <header className="space-y-4">
                <div className="flex items-center gap-4">
                  <span className="px-4 py-1.5 rounded-full bg-primary/10 text-primary text-label-caps !text-[9px] border border-primary/20">
                    {getFeatureLabel(activeSubmission.taskId)} Review
                  </span>
                  <span className="text-label-caps !text-[9px] text-white/20">
                    {timeAgo(activeSubmission.submittedAt)}
                  </span>
                </div>
                <h1 className="text-5xl md:text-7xl lg:text-8xl text-hero text-white !normal-case leading-[0.8] tracking-tight">
                  {activeSubmission.registration.teamName}.
                </h1>

                 {/* Links */}
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4 pt-2">
                  <div className="space-y-1.5">
                    <span className="text-[8px] text-white/20 uppercase font-mono block">GitHub Commit Link</span>
                    <a
                      href={activeSubmission.payload?.github}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-sm text-emerald-400 underline break-all font-mono hover:text-emerald-300 transition-colors"
                    >
                      {activeSubmission.payload?.github || 'Not provided'}
                    </a>
                  </div>
                  <div className="space-y-1.5">
                    <span className="text-[8px] text-white/20 uppercase font-mono block">Loom / Demo Video Link</span>
                    <a
                      href={activeSubmission.payload?.liveDemo}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-sm text-sky-400 underline break-all font-mono hover:text-sky-300 transition-colors"
                    >
                      {activeSubmission.payload?.liveDemo || 'Not provided'}
                    </a>
                  </div>
                </div>
                {activeSubmission.payload?.description && (
                  <div className="space-y-1.5 pt-4 border-t border-white/5 mt-4">
                    <span className="text-[8px] text-white/20 uppercase font-mono block">Team Description / Notes</span>
                    <p className="text-xs text-white/70 leading-relaxed bg-white/[0.02] border border-white/5 rounded-xl p-4 font-mono whitespace-pre-wrap">
                      {activeSubmission.payload.description}
                    </p>
                  </div>
                )}
              </header>
 
              {/* Scoring form */}
              <div className="glass-premium p-6 md:p-10 lg:p-12 rounded-2xl md:rounded-[3rem] border-white/5 space-y-8 md:space-y-10 shadow-[0_40px_100px_-20px_rgba(0,0,0,0.8)]">
                <form onSubmit={handleGradeSubmit} className="space-y-8">
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-x-12 gap-y-4">
                    {currentCriteria.map(({ key, label, desc, max }: any) => (
                      <div key={key} className="space-y-1 group">
                        <div className="flex items-end justify-between">
                          <div>
                            <h4 className="text-label-caps !text-[10px] text-white/60 mb-0.5 group-hover:text-white transition-colors">
                              {label}
                            </h4>
                            <p className="text-label-caps !text-[7px] opacity-20">{desc}</p>
                          </div>
                          <div className="flex items-center gap-2">
                            <input
                              type="number"
                              min={0}
                              max={max}
                              value={scores[key]}
                              onChange={(e) => {
                                const val = Math.max(0, Math.min(max, Math.round(Number(e.target.value) || 0)))
                                setScores((prev) => ({ ...prev, [key]: val }))
                              }}
                              className="w-16 bg-white/5 border border-white/10 rounded-lg px-2 py-1 text-right text-lg text-primary font-mono focus:outline-none focus:border-primary/50 focus:ring-1 focus:ring-primary/20"
                            />
                            <span className="text-xs text-white/30 font-mono">/ {max}</span>
                          </div>
                        </div>
                        <Slider.Root
                          value={Number(scores[key]) || 0}
                          onValueChange={(val) =>
                            setScores((prev) => ({ ...prev, [key]: Math.round(Number(val) || 0) }))
                          }
                          min={0}
                          max={max}
                          step={1}
                        >
                          <Slider.Control className={sliderStyles.Control}>
                            <Slider.Track className={sliderStyles.Track}>
                              <Slider.Indicator className={sliderStyles.Indicator} />
                              <Slider.Thumb aria-label={label} className={sliderStyles.Thumb} />
                            </Slider.Track>
                          </Slider.Control>
                        </Slider.Root>
                        <div className="flex justify-between text-value-mono !text-[7px] !text-white/10 font-bold">
                          <span>0</span>
                          <span>{max}</span>
                        </div>
                      </div>
                    ))}
                  </div>
 
                  {/* Overall Score */}
                  {currentCriteria.length > 0 && (
                    <div className="relative overflow-hidden p-6 md:p-8 rounded-2xl md:rounded-[2rem] bg-primary/5 border border-primary/20 flex flex-col md:flex-row items-center justify-between gap-4 group transition-all hover:bg-primary/10">
                      <div className="absolute top-0 left-0 w-full h-full neural-grid opacity-[0.05]" />
                      <div className="relative z-10">
                        <span className="text-label-caps !text-primary !text-[9px]">Overall Rating</span>
                        <h3 className="text-2xl text-hero !normal-case !tracking-tight !text-white mt-1">Total Score</h3>
                      </div>
                      <div className="relative z-10 text-right">
                        <span className="text-6xl text-stat !text-primary group-hover:scale-105 transition-transform block leading-none font-mono">
                          {overall}
                        </span>
                        <span className="text-label-caps !text-[10px] opacity-40 mt-1 block">/ {
                          currentCriteria.reduce((sum: number, c: any) => sum + (c.max || 0), 0)
                        }</span>
                      </div>
                    </div>
                  )}

                  {/* Feedback */}
                  <div className="space-y-3">
                    <label className="text-label-caps !text-[9px] text-white/30 px-2 italic">Judge Feedback</label>
                    <textarea
                      required
                      className="w-full bg-white/[0.03] border border-white/5 rounded-[1.5rem] p-5 text-sm text-editorial focus:outline-none focus:border-primary/50 focus:bg-white/[0.06] transition-all min-h-[100px] placeholder:text-white/10 shadow-inner"
                      placeholder="Enter technical feedback and recommendations for the team..."
                      value={notes}
                      onChange={(e) => setNotes(e.target.value)}
                    />
                  </div>

                  {/* Submit */}
                  <div className="space-y-4 pt-2">
                    <button
                      type="submit"
                      disabled={submitLoading}
                      className="w-full py-4 rounded-[1.5rem] bg-white text-black text-label-caps !text-[11px] transition-all hover:scale-[1.01] active:scale-[0.99] shadow-2xl font-black disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer hover:bg-primary hover:text-white"
                    >
                      {submitLoading ? 'SUBMITTING...' : (currentCriteria.length > 0 ? 'SUBMIT GRADE & EVALUATION' : 'SUBMIT FEEDBACK')}
                    </button>
                  </div>
                </form>
              </div>
            </motion.div>
          ) : (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="h-full flex flex-col items-center justify-center text-center p-6 md:p-12 lg:p-24"
            >
              <div className="w-28 h-28 rounded-[2rem] border border-white/5 flex items-center justify-center mb-8 relative group bg-white/[0.02] shadow-2xl">
                <div className="absolute inset-0 rounded-[2rem] bg-primary/20 blur-3xl opacity-0 group-hover:opacity-100 transition-opacity" />
                <span className="text-5xl grayscale group-hover:grayscale-0 transition-all scale-110">⚖️</span>
              </div>
              <h2 className="text-[9px] font-black text-white/20 uppercase tracking-[0.5em] mb-4">Ready to Judge</h2>
              <h3 className="text-3xl md:text-5xl text-hero !normal-case !tracking-tight text-white leading-tight">
                Select a team from the queue<br />
                <span className="text-white/20 italic">to begin scoring.</span>
              </h3>
              <p className="mt-5 text-sm text-white/30 max-w-md font-medium text-editorial leading-relaxed">
                Review their GitHub code and demo, score across 7 criteria, then approve to unlock the next stage or request changes.
              </p>
            </motion.div>
          )}
        </AnimatePresence>
        )}

      </div>
    </main>
  )
}
