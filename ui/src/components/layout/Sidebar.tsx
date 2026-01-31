/**
 * DiMA Desktop - Sidebar Component
 * 
 * Main navigation sidebar with projects list, settings, and about sections.
 * Collapsible to icons-only mode.
 */

import { useState } from 'react';
import { 
  FolderOpen, 
  Plus, 
  Settings, 
  Info, 
  ChevronLeft, 
  ChevronRight,
  Dna,
  Clock,
  Trash2,
  SquareSplitHorizontal,
} from 'lucide-react';
import { useAppStore } from '@/stores/appStore';
import { useProjectStore } from '@/stores/projectStore';
import { createNewWindow } from '@/lib/tauri';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { Separator } from '@/components/ui/separator';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

export function Sidebar() {
  const { 
    sidebarCollapsed, 
    toggleSidebar, 
    recentProjects, 
    currentView,
    setCurrentView,
    clearRecentProjects,
  } = useAppStore();
  
  const { createNewProject, openExistingProject, currentProject } = useProjectStore();
  const [isCreating, setIsCreating] = useState(false);
  const [showNameDialog, setShowNameDialog] = useState(false);
  const [projectName, setProjectName] = useState('');
  const [nameError, setNameError] = useState<string | null>(null);

  const generateDefaultName = () => {
    return `Analysis ${new Date().toISOString().slice(0, 19).replace('T', ' ')}`;
  };

  const handleOpenNameDialog = () => {
    setProjectName(generateDefaultName());
    setNameError(null);
    setShowNameDialog(true);
  };

  const handleCreateProject = async () => {
    const trimmedName = projectName.trim();
    if (!trimmedName) {
      setNameError('Project name is required');
      return;
    }

    try {
      setIsCreating(true);
      setShowNameDialog(false);
      await createNewProject(trimmedName);
      setCurrentView('wizard');
    } catch (error) {
      console.error('Failed to create project:', error);
      setNameError(String(error));
      setShowNameDialog(true);
    } finally {
      setIsCreating(false);
    }
  };

  const handleOpenProject = async (path: string) => {
    try {
      await openExistingProject(path);
      setCurrentView('wizard');
    } catch (error) {
      console.error('Failed to open project:', error);
    }
  };

  return (
    <TooltipProvider delayDuration={100}>
      <div 
        className={cn(
          "flex flex-col border-r bg-card transition-all duration-300",
          sidebarCollapsed ? "w-16" : "w-[350px]"
        )}
      >
        {/* Header */}
        <div className="flex h-14 items-center border-b px-4">
          {!sidebarCollapsed && (
            <div className="flex items-center gap-2">
              <Dna className="h-6 w-6 text-primary" />
              <span className="font-semibold">DiMA Desktop</span>
            </div>
          )}
          {sidebarCollapsed && (
            <Dna className="mx-auto h-6 w-6 text-primary" />
          )}
        </div>

        {/* New Analysis Button */}
        <div className="p-3">
          {sidebarCollapsed ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button 
                  size="icon" 
                  className="w-full"
                  onClick={handleOpenNameDialog}
                  disabled={isCreating}
                >
                  <Plus className="h-5 w-5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="right">New Analysis</TooltipContent>
            </Tooltip>
          ) : (
            <Button 
              className="w-full gap-2" 
              onClick={handleOpenNameDialog}
              disabled={isCreating}
            >
              <Plus className="h-4 w-4" />
              New Analysis
            </Button>
          )}
        </div>

        <Separator />

        {/* Projects Section */}
        <div className="flex-1 overflow-hidden">
          {!sidebarCollapsed && (
            <div className="flex items-center justify-between px-4 py-2">
              <div className="flex items-center gap-2 text-sm font-medium text-muted-foreground">
                <Clock className="h-4 w-4" />
                Recent Projects
              </div>
              {recentProjects.length > 0 && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <button
                      onClick={clearRecentProjects}
                      className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </TooltipTrigger>
                  <TooltipContent side="right">Clear all recent projects</TooltipContent>
                </Tooltip>
              )}
            </div>
          )}
          
          <ScrollArea className="h-full">
            <div className="space-y-1 p-2">
              {recentProjects.length === 0 ? (
                !sidebarCollapsed && (
                  <p className="px-2 py-4 text-center text-sm text-muted-foreground">
                    No recent projects
                  </p>
                )
              ) : (
                recentProjects.map((project) => (
                  <Tooltip key={project.path}>
                    <TooltipTrigger asChild>
                      <button
                        onClick={() => handleOpenProject(project.path)}
                        className={cn(
                          "flex w-full items-center gap-3 rounded-md px-3 py-2 text-left transition-colors hover:bg-accent",
                          currentProject?.path === project.path && "bg-accent"
                        )}
                      >
                        <FolderOpen className="h-4 w-4 shrink-0 text-muted-foreground" />
                        {!sidebarCollapsed && (
                          <div className="min-w-0 flex-1">
                            <p className="truncate text-sm font-medium">
                              {project.name}
                            </p>
                            {project.input_file_name && (
                              <p className="truncate text-xs text-muted-foreground">
                                {project.input_file_name}
                              </p>
                            )}
                          </div>
                        )}
                      </button>
                    </TooltipTrigger>
                    {sidebarCollapsed && (
                      <TooltipContent side="right">
                        {project.name}
                      </TooltipContent>
                    )}
                  </Tooltip>
                ))
              )}
            </div>
          </ScrollArea>
        </div>

        <Separator />

        {/* Bottom Actions */}
        <div className="space-y-1 p-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={() => createNewWindow()}
                className="flex w-full items-center gap-3 rounded-md px-3 py-2 transition-colors hover:bg-accent"
              >
                <SquareSplitHorizontal className="h-4 w-4 shrink-0" />
                {!sidebarCollapsed && <span className="text-sm">New Window</span>}
              </button>
            </TooltipTrigger>
            {sidebarCollapsed && (
              <TooltipContent side="right">New Window</TooltipContent>
            )}
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={() => setCurrentView('settings')}
                className={cn(
                  "flex w-full items-center gap-3 rounded-md px-3 py-2 transition-colors hover:bg-accent",
                  currentView === 'settings' && "bg-accent"
                )}
              >
                <Settings className="h-4 w-4 shrink-0" />
                {!sidebarCollapsed && <span className="text-sm">Settings</span>}
              </button>
            </TooltipTrigger>
            {sidebarCollapsed && (
              <TooltipContent side="right">Settings</TooltipContent>
            )}
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={() => setCurrentView('about')}
                className={cn(
                  "flex w-full items-center gap-3 rounded-md px-3 py-2 transition-colors hover:bg-accent",
                  currentView === 'about' && "bg-accent"
                )}
              >
                <Info className="h-4 w-4 shrink-0" />
                {!sidebarCollapsed && <span className="text-sm">About</span>}
              </button>
            </TooltipTrigger>
            {sidebarCollapsed && (
              <TooltipContent side="right">About</TooltipContent>
            )}
          </Tooltip>
        </div>

        {/* Collapse Toggle */}
        <div className="border-t p-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                onClick={toggleSidebar}
                className="w-full"
              >
                {sidebarCollapsed ? (
                  <ChevronRight className="h-4 w-4" />
                ) : (
                  <>
                    <ChevronLeft className="h-4 w-4" />
                    <span className="ml-2">Collapse</span>
                  </>
                )}
              </Button>
            </TooltipTrigger>
            {sidebarCollapsed && (
              <TooltipContent side="right">Expand sidebar</TooltipContent>
            )}
          </Tooltip>
        </div>

        {/* New Project Name Dialog */}
        <Dialog open={showNameDialog} onOpenChange={setShowNameDialog}>
          <DialogContent className="sm:max-w-[425px]">
            <DialogHeader>
              <DialogTitle>Create New Analysis</DialogTitle>
              <DialogDescription>
                Enter a name for your new analysis project.
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              <div className="grid gap-2">
                <Label htmlFor="project-name">Project Name</Label>
                <Input
                  id="project-name"
                  value={projectName}
                  onChange={(e) => {
                    setProjectName(e.target.value);
                    setNameError(null);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      handleCreateProject();
                    }
                  }}
                  placeholder="Enter project name..."
                  autoFocus
                />
                {nameError && (
                  <p className="text-sm text-destructive">{nameError}</p>
                )}
              </div>
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={() => setShowNameDialog(false)}>
                Cancel
              </Button>
              <Button onClick={handleCreateProject} disabled={isCreating}>
                {isCreating ? 'Creating...' : 'Create'}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>
    </TooltipProvider>
  );
}
